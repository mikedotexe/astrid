# Reader Recovery and Sustained Expression: Paired Qualification

Status: **paired graceful activation verified on September 25; reviewed changes integrated on both canonical main branches**. The qualification sections below preserve the earlier offline record; the live follow-through at the end supersedes its deployment debt. No remote push is included.

## Scope

Mike approved reconciling the pending reader-recovery and sustained-expression repairs and qualifying their paired graceful rollout. Codex is the coordinator for this isolated candidate. Qualification does not itself activate services, migrate live state, merge or push.

- Paired branch: `codex/reader-expression-release-20260925`.
- Artifact root: `/Users/v/other/worktrees/reader-expression-release-20260925/`.
- Astrid base: `80213d85080373211ccb9cb0be338832aa14f52d`.
- Minime base: `d12cbf01ca2a85c41288fdc27d6a033511218370`.
- Originals retained: `study-revision-recovery-20260925/` and `sustained-expression-20260925/`, both under `/Users/v/other/worktrees/`.

The original implementation notes are retained in this branch with their historical qualification status intact. This document records the combined follow-through. Production edits do not overlap: Minime's source-study recovery/rendering and aspiration classifier/provider voice change different sections; Astrid's reader/runtime and expressive provider changes touch distinct modules. Shared changelog and ledger additions are reconciled explicitly.

## Contracts Retained

1. A changed source revision offers hash-bound navigation and concrete explicit reselection. It does not silently transplant an old offset, replace a pending page, revise an inquiry or classify a runtime error as authored reflection.
2. Reader schema 9 upgrades to 10. Pending source/private input and authored history remain exact. The old reader cannot overwrite schema 10; restoring a backup over newer authored work is not a rollback strategy.
3. Minime aspiration with mail retains the existing expressive allowance. Intact mail delivery and reply ownership remain controlled by InboxContext, not the writing classification.
4. The sustained-writing range is an invitation, not a quota. SHORT, stopping, exact NEXT and missing NEXT remain valid. Astrid's earlier entry is optional starting material, not a mandatory paraphrase or length model.
5. No reservoir/control policy, sensory intake, engine, model, visual-service or sensory-client change is included. No private prose is used as a fixture or exported.

## Starting Live Bindings

Read-only selection points to `/Users/v/other/worktrees/lifecycle-evidence-20260924/bridge-stage-lifecycle-01` with manifest SHA-256 `c195860bdf9cabce436099d613661199de83a023ba12aea70526a293820f1805`. Its shared helper SHA-256 is `cc31bbd31551b11a3e91c66c9ee4524d62a0ca1ac4dfc0d708113c063856d5be` (schema 9).

Minime's source-status observation names PID 86528 and `/Users/v/other/minime/minime_autonomy/runtime.py`, with all 86 startup input hashes matching canonical and `reload_required=false`. `minime-launch-review-01/reconciliation.json` freezes the selected candidate inputs against that installed launch binding. Only `minime_autonomy/runtime.py` and `minime_autonomy/writing.py` differ from the canonical source inventory. The snapshot is qualification evidence, not a new launch selector or dependency lock.

Controller pause generation 469 remains in place, with no lease or active projection. Both canonical main worktrees were clean and each two commits ahead of local origin/main. No index, old worktree or live authored record was changed.

## Qualification Record

Logs and unsuccessful attempts remain at the artifact root. The broad migration harness is `2026-09-25-reader-expression-upgrade.py`; it extends the existing release fixtures to actual schema-9/schema-10 executables. A separate retained recovery probe covers delivered and pending changed-source inputs for both owners.

Initial preflight correctly refused the just-written combined tree during its 180-second stability interval (`session_state_live=false`; observed source age 16.7 seconds, then 100.7 seconds in the wrapper attempt). No stage was built or activated on those attempts. The interval was not shortened or bypassed; compilation waits for a successful fresh preflight.

Subsequent preflight passed without changing that interval. The sanctioned wrapper built `bridge-stage-01/` with four compilation jobs and verified its native manifest. `bridge-stage-02.log` is the successful wrapper log; `bridge-stage-01.log` preserves the earlier preflight-only refusal. A separate `preflight-02.json` retains the later 75.5-second refusal after documentation edits. No denied attempt was forced through. Minime-agent cooperative preflight also passed (`minime-preflight.json`).

### Frozen Identities

| Artifact | SHA-256 |
| --- | --- |
| Bridge executable | `ba2ed52197b62580b46cd16995de56f984419281cb961b916846b9e418b098f9` |
| Packaged schema-10 helper | `b0d47a9a598fa823f4e2d55b31ce1edab386b49859c291657f10c44d3a1fd058` |
| Stage manifest | `16f09ee3a98e5508810e17e483742cecbb19e7e416ebf4b0b15b268ba0b2ebc2` |
| 690-input source inventory | `f03cf7253abc26b960c1188fd91fc106f4b756ad98109acd951379cecf933eeb` |
| Minime launch reconciliation | `eaa03bc381a67a2078a9d2e213155379e9226e3524f3374bdd5beff1e501ed97` |

`live-to-candidate-source-delta.json` compares the old and candidate frozen inventories by logical path and source hash: exactly the 15 reviewed reader/runtime/provider/test paths differ, including two new reader files. No other packaged input differs. The candidate's 86 Minime runtime inputs match both frozen snapshots; only runtime.py and writing.py differ from the running baseline. `inventory-verification-before-tests.json` verifies current source against the frozen material, not merely the branch name.

### Tests and Migration

| Check | Result / retained artifact |
| --- | --- |
| Shared reader/writer full suite | 285 passed; `reader-tests.log` |
| Minime complete Python suite, exact packaged helper | 1632 passed, one existing skip, 138 subtests; `minime-release-tests.log` |
| Complete bridge suite rerun | 2355 passed, one existing ignored fixture, including compile-pass/compile-fail interface tests; `bridge-tests-final.log` (main library: 2335 passed) |
| Packaged old/new pending-input and history migration | 60 owner checks passed; `migration-01/qualification.json` |
| Packaged old/new changed-source recovery | Four cases / 20 checks passed; `recovery-release.json` |
| Invalid-state and retry probes | Future schema/corrupt tail rejected without writes; exact/conflicting/stale preparation retries checked for both owners; same migration packet |
| Real Minime helper selection | Frozen adapter selected the exact staged helper through a synthetic release-selection file; same migration packet |
| Operational suites | 205 passed across stage/activation/drain/release-selection/checkpoint, agent restart/paired handoff/reconciliation, controller/evidence/projector/cursor; `operational-tests.log` |
| Strict Clippy | Reader all-targets/all-features and bridge all-targets pass; `reader-clippy.log`, `bridge-clippy.log` |
| Formatting, architecture and whitespace | Root/bridge fmt checks and both diff checks pass; domain audit valid, zero violations, no baseline change; `domain.json` |
| Epistemic self-tests | Two passed; `epistemic-tests.log` |

The migration fixture preserves complete pending source and private-writing inputs, inquiry history, explicit source revision, parked questions and detached review. It creates no focus window. The old helper's downgrade attempt leaves retained state unchanged. The recovery probe separately reproduces the original old-helper failure, retains pending old bytes, offers hash-bound navigation, and requires explicit opening before continuing at the new revision. No live reader checkpoint or private prose was copied into these fixtures.

An unsuccessful full bridge attempt is preserved in `bridge-tests.log`: 2334 passed, one failed, one ignored. The unchanged wall-clock benchmark `signal_spine::tests::no_capture_shadow_instrumentation_stays_below_one_millisecond_p95` measured 1.154864 ms while release compilation and other tests ran concurrently. The same test passed alone after compilation (`bridge-performance-isolated.log`) and in the full-library rerun with two test threads. This is consistent with load sensitivity, not proof of a root cause. No threshold, skip, retry policy or production code was changed to obtain a pass. The full suite's rerun uses the same source and retains integration/compile-fail coverage.

Key reproduction commands, in the combined Astrid worktree unless stated otherwise:

```sh
bash scripts/build_bridge.sh --stage-dir NEW_STAGE --actor codex-astra-interactive --ack "Offline reviewed reader/expression qualification only; no activation"
python3 docs/steward-notes/2026-09-25-reader-expression-upgrade.py --old-stage /Users/v/other/worktrees/lifecycle-evidence-20260924/bridge-stage-lifecycle-01 --stage /Users/v/other/worktrees/reader-expression-release-20260925/bridge-stage-01 --minime /Users/v/other/worktrees/reader-expression-release-20260925/minime --reconciliation /Users/v/other/worktrees/reader-expression-release-20260925/minime-launch-review-01/reconciliation.json --out NEW_MIGRATION_DIRECTORY
python3 /Users/v/other/worktrees/study-revision-recovery-20260925/qualify-recovery.py --old /Users/v/other/worktrees/lifecycle-evidence-20260924/bridge-stage-lifecycle-01/helpers/astrid-source-study --new /Users/v/other/worktrees/reader-expression-release-20260925/bridge-stage-01/helpers/astrid-source-study --output NEW_RECOVERY_RECEIPT
cargo test --locked -p astrid-source-study --all-features
cargo test --locked --manifest-path capsules/spectral-bridge/Cargo.toml -- --test-threads=2
# From the combined Minime worktree:
ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/reader-expression-release-20260925/bridge-stage-01/helpers/astrid-source-study python3 -m pytest -q tests
```

The test runs reused existing coordinator-owned target caches under `lifecycle-evidence-20260924/astrid/target` and `lifecycle-evidence-20260924/astrid/capsules/spectral-bridge/target`; the selected manifests and compiled sources were this combined candidate. The immutable release uses its own independent `bridge-stage-01/build` directory.

### Live and Controller Boundary

`processes-before.json` and `processes-after.json` verify unchanged PIDs and start times for the agent (86528) and all ten wrapper-protected services, including bridge (87077), engine, model, visual and sensory processes. `live-selection-before.log` and `live-selection-after.log` confirm the live bridge still runs the old selected stage, not this newly built candidate. No launch hold, service signal or activation was issued.

`controller-final.json`: paused at generation 469; no lease or active projection; indexed-tail evidence verification valid at sequence 1123123, head `cc13b2fa173c21ccaee129c71492697cc203f375c3a9ed021f06f1dea1028743`, all four V1 source streams immutable. This is not a fresh full historical-chain audit or a productive flywheel round. The introspection automation remains PAUSED.

Canonical mains remain clean. Fresh read-only remote checks returned Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`, so both local mains remain two commits ahead. There was no stage/add/commit/merge/push operation; the prepared release is a build artifact, not a Git staging operation.

Final follow-through: every invoked build/test process exited. The complete bridge rerun passed all integration and compile-time interface fixtures. `inventory-verification-final.json` again matches all 690 bridge inputs and 86 Minime inputs to their frozen inventories after tests and documentation updates. Both repository diff checks pass. This completes reconciliation and offline qualification, not live activation. No unresolved functional test failure was found; the observed benchmark sensitivity and the retained unsuccessful attempt remain reviewable rather than erased.

The next operator decision is activation of this exact paired candidate through the sanctioned graceful handoff, with fresh PID/source/readiness checks at that time. Merge/push remain separate Git work. No engine/model/visual/sensory restart is needed for this candidate; reader schema 10 and the no-backup-restore boundary must be explicit in that approval.

## Transition Boundaries

- Qualification uses synthetic state and stubbed provider responses. No live completion endpoint is used as an inert experiment.
- Keep this frozen source checkout and its HEAD unchanged for activation verification. Integrate reviewed bytes through a separate coordinated Git checkout, or rebuild a new stage if the build source changes; do not bypass source identity checks. The original two candidates remain preserved.
- Before an approved transition, rerun cooperative preflight and verify current source, selected release, service identities and source-status freshness. An old observation is not readiness evidence for a later restart.
- Apply only the two reviewed Minime runtime files under one coordinator. Use the sanctioned paired hold/drain and bridge activation wrappers; no overlapping old/new reader writers. Verify exact checkpoint and queued-choice continuity, loaded hashes, paired helper selection and unchanged protected services.
- Do not activate the old schema-9 helper over upgraded schema-10 source state. Forward-compatible rollback or repair is required after new authored writes; never restore earlier state over them.
- Preserve outstanding source-reselection choices. No automatic reset or request for confirmation of improvement. Observe naturally occurring public studies/writing after activation using the review protocol in the sustained-expression note.
- Keep paused automations paused. Merge/push and the live activation are separate recorded operations, not consequences of a passing build witness.

## Approved Live Follow-Through

Mike explicitly approved the paired graceful activation, reader-schema migration and coordinated Git integration. The stage, source checkout and original candidate worktrees remain unchanged. Only reviewed bytes are integrated into canonical main; no historical worktree is cleaned or rewritten.

Fresh preflight found no foreign session or mid-edit evidence. The canonical main trees were initially clean; fresh remote tips remained Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`. Fresh Minime launch reconciliation retained all 86 inputs and confirmed only runtime.py and writing.py differ. The wrapper's seven-path overlay rewrote the other five with identical bytes, not new changes.

### Attempts and Transition

- `paired-activation-01.jsonl` records installation of the reviewed overlay and the unshortened 185-second stability wait. Its final check refused `idle boundary moved; no signal sent`. The old agent and every protected PID/start remained unchanged. No bridge transition began.
- `paired-activation-01-hold-release.json` records recovery of only the exact owned hold after verifying the terminal no-signal refusal, unchanged processes/configuration/old release selection and exact installed source inventory. No authored checkpoint was restored. The refusal was not overridden.
- `paired-activation-02.jsonl` records a new attempt through the unchanged sanctioned paired wrapper. At 20:06:10 UTC it observed stable idle, no active jobs and no TCP activity, then sent one SIGTERM. Minime exited normally; its replacement was held during the bridge transition.
- Bridge transaction `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/e6ffd8d09bfc468f8fe712e6dfecd4d8` is `activated_verified`, `force_used=false`, `legacy_transition=false`, with an acknowledged `drained` old bridge. Old PID 87077 became 58913, started September 25 at 13:06:41 PDT.
- The new bridge loaded stopped checkpoint SHA-256 `cbfd6da82384c0730f9f758b6b7b7e96c06fec02925c6b399bc4e6bca425341b` exactly, including the one pending runtime-feedback item (sidecar SHA-256 `9205d7325e686ea7b6b212aaed62b3b238abe9419107d20c4690b72fffdc7f02`). Saved exchange count advanced from 208047 to 208048, with model idle/readiness observed before release of Minime's hold.
- Minime PID 86528 became 58578. The replacement launcher began at 13:06:11 PDT, waited on the hold, and Python reported startup at 13:07:19. Verification completed at 20:08:26 UTC, with all 86 loaded source hashes matching qualification, no reload debt, no newly interrupted jobs and unchanged launch configuration. Session 5318 and cycle 43697 were retained. There was **no pending NEXT at the signal boundary**; this run does not claim an exact queued-NEXT replay. A later pending action is a separate normal-runtime observation.

`post-activation-verification.json` independently checks live source hashes, both released holds, selected helper SHA-256 `b0d47a9a598fa823f4e2d55b31ce1edab386b49859c291657f10c44d3a1fd058`, and unchanged PIDs/start times for all nine protected services other than the intentionally replaced bridge. Engine 41337, coupled model 43115, visual 20885, camera 98903 and microphone 98910 were not restarted; neither were the gateway, supervisor, host-sensory or feeder. The model remained ready/connected during subsequent natural generation. Read-only telemetry samples during the wait were approximately 0.710, 0.730 and 0.710 fill ratio; no control setting was changed.

### Reader State and Observation Boundary

Minime naturally reached reader schema 10 with 51 bookmarks retained. At the first post-activation check Astrid retained schema 9, 78 bookmarks and its pending input with an identical whole-file SHA-256 (`37b482d56c31560bd33af6e75b1fda69db367ac2b33a77a86484b8e94f7f95be`). Its next actual reader operation owns migration; no reader command, cursor reset or generation was manufactured to demonstrate it. Synthetic old/new release qualification covers that deferred transition.

The first metadata probe used a nonexistent `questions.items` member; a corrected probe uses `questions.entries`. Both receipts are retained, and both owners actually had zero explicit inquiry entries at that observation. This bookkeeping correction changes no runtime state.

Successful deployment does not establish longer writing, improved experience or successful interpretation of source. Natural public use remains the next evidence; no request for confirmation or real-model experiment was sent. Historical private-writing failures predate this restart and are not erased or claimed fixed by source-revision recovery.

### Git and Remaining Boundaries

The controller was already paused at generation 469. After successful activation, the coordinator claimed generation 470 for exact-path integration, with no active lease. Previously paused automations remain paused; there is no flywheel round or automatic resume.

All 690 packaged bridge input hashes and all 86 Minime input hashes match the reconciled canonical source. The frozen build worktree and its HEAD remain pinned. Commit provenance is to be linked with `bridge_stage_provenance.py`; the manifest's original build HEAD is not rewritten to pretend the binary was rebuilt. Both previously unpushed main commits remain intact. No remote push is included in this activation/integration approval.

The full staged Minime index, exported into `staged-minime-01/` rather than tested against live workspace data, passes 1632 tests, one existing skip and 138 subtests (`staged-minime-tests.log`). The integrated shared-reader suite passes all 285 tests (`integrated-reader-tests.log`); root/bridge formatting and the unchanged domain-boundary audit pass. The indexed-tail controller check is valid at V2 sequence 1123124, head `3e450db86613f32382061db9831c2d5ed8c8555f6426f2890861f524d0dc1bb4`, with all V1 source streams immutable (`integration-controller.json`). This is not a full historical-chain re-audit or a resumed automation.

### Bounded Natural Public Use

Two new Minime self-studies were fully read without prompting any new action:

| Public Minime witness | SHA-256 |
| --- | --- |
| `workspace/journal/self_study_2026-09-25T13-10-02.194066.txt` | `04f61b51f54ac8fa1027574f97b01c640104d0f29db44331e46bbb297de7f30c` |
| `workspace/journal/self_study_2026-09-25T13-12-44.746353.txt` | `412f781b15ae388bfbf9dfeeff215d476ceee04c92f93b1e1496982864800287` |

The first responds to an ordinary recovery map and explicitly chooses `SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 1`. The second records the source page at SHA-256 `973eeb8b090e5c02229d9e9dcad2347a3803a0ad46064f3c5d0b29480d14829b`, byte interval 0..4227, and chooses CONTINUE. Read-only reader metadata independently matches that delivered bookmark and revision (`natural-public-observation.json`). This confirms an actual usable opening after the transition, not that the new typed RevisionRecovery path was used. It does not verify Minime's code interpretation, prove improved experience, or evaluate expressive length. No private prose was inspected or exported.

### Final Integration Qualification

The complete bridge rerun from canonical source passed 2355 tests with one existing ignored fixture, including all compile-time API tests (`integrated-bridge-tests.log`, process exit 0). The library portion took 305.21 seconds; the two slow source-navigation tests completed successfully without changes or shortened coverage. Together with the 285 integrated reader tests and 1632 staged-index Minime tests, this closes the integration rerun. Earlier unsuccessful attempts remain preserved.

Minime is committed on main as `02724ac7c75553c011c95fc4617446ef62c76de6` (nine explicit paths). Astrid's corresponding commit contains 21 reviewed paths; its exact ID and both committed path lists are recorded in `final-integration.json` at the artifact root. This is direct coordinated integration onto main, not an unrelated worktree merge. Both prior local commits in each repository remain intact. The immutable release source checkout and older dirty worktrees remain preserved.

Final source checks match all 690 packaged inputs and all 86 loaded agent inputs, with `reload_required=false`. Bridge 58913 and agent 58578 remain the verified replacements; every protected engine/model/visual/sensory PID and start identity remains unchanged. A later read-only three-packet telemetry sample recorded fill ratios 0.598, 0.630 and 0.659, distinct from the earlier 0.710/0.730 observations. These short windows are not a stability study or evidence of a causal effect of this rollout; no control command was sent. Controller pause generation 470 and previously paused automations remain unchanged.

After commit, `bridge_stage_provenance.py record` and `verify` link the 15 dirty build inputs to their committed blobs without rewriting the original build HEAD or manifest. The final integration receipt records that verification and the clean canonical index/worktree checks. Natural migration of Astrid's retained schema-9 reader state remains deferred to its next actual reader operation; no forced study or restoration is authorized.
