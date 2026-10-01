# Coupled Covariance and Controller Qualification

Follow-through: the [measurement-basis repair](2026-09-30-measurement-basis-repair.md) fixes the defect and resolves the scoped lint debt identified below. This packet and its receipts retain their original pre-repair results; the new packet records current qualification and remaining release gates.

## Decision

**The bounded offline coupling study is complete. Engine deployment is not qualified.** The actual production structural controller now shares its covariance update and scaffold lifecycle with a synthetic, explicit-clock harness. Delayed-clock comparisons produce different controller decisions and different covariance matrices. They do not produce different reported fill trajectories in this corpus. Qualification also reproduces a separate measurement-basis recovery defect that warrants repair before an engine rollout.

Mike authorized this follow-through to the [availability-aware timing repair](2026-09-30-fill-rate-validity-repair.md). All changes remain uncommitted in `/Users/v/other/worktrees/afterimage-timing-20260930/{astrid,minime}`, branch `codex/afterimage-timing-20260930`, based on Minime `e9f2f5f151c89dd6b4a2dc80d5d8d12a60dc20d3` and the preceding owned review/repair delta. No service, model, engine, live configuration, checkpoint or authored state was changed. Previously paused automations remain paused.

Date: September 30, 2026, America/Los_Angeles; host UTC may already be October 1. Steward: Codex interactive collaborator.

## Witness

Astrid's public `capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt` remains unchanged, SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`, reverified in this pass. Its incomplete-trace account motivated inspecting the archive and numerical clock, not a conclusion that the controller caused the account.

The earlier reproduction remains: 74.2541% to 72.1279% over 2.37 seconds is approximately -0.8971 percentage points/s, not -4.2523 from dividing by a nominal 0.5 seconds. The archived cadence shortfall is still explicit. This work neither fills missing samples nor changes a sensory admission policy.

## Production Changes

### Shared Step, Not a Replacement Plant

Minime's new `minime/src/covariance_math.rs` contains the existing rank-one, decay and identity-reset functions, moved out of the runtime include. Arithmetic order and trace normalization are retained; the rank-one loop uses ordered iterator bindings to avoid adding a Clippy diagnostic.

`minime/src/stable_covariance.rs` owns the actual structural PI-to-matrix step and post-measurement scaffold lifecycle. The engine and harness both call it. Inputs include finite-or-unavailable fill/rate, stage/guard, scaffold, projected stimulus, pressure preference and explicit timestamp. Outputs retain the matrix mutation, operational mode, drain/live weights, restart gate fields, reset and skipped-input outcomes. No I/O, GPU handles or ambient clock reads occur inside these functions.

The extraction retains ordinary scaffold blending, reentry, recovery impulse, legacy low-fill escape, free rank-one/decay updates, absolute restart drain floors, retirement and activation order. It does not retune gains, thresholds or tick-count durations. Runtime orchestration still owns the GPU buffers, observation clocks, input lifecycle and telemetry. Several near-adjacent wall-clock reads become one supplied timestamp per step/lifecycle call; exact behavior at a millisecond threshold boundary is not claimed identical to the former multiple reads.

### Additional Reset-Boundary Repair

Review found that rank-one and decay wrappers could internally reset a corrupt matrix without reporting the reset to the fill clocks. Those wrappers now return the reset outcome. The shared structural step also exposes internal numerical resets. Runtime invalidates both measurement and regulation clocks for these resets, as well as the previously covered explicit recovery resets. A reset log and nonfinite-input skip log remain available.

This is a real additional candidate behavior change: a derivative may not straddle a covariance replacement. It is not a live change. The matrix-fault fixture and focused tests exercise rank-one and decay reset reporting and unavailable post-reset rates.

### Explicit Estimator Clock

`EigenFillEstimator::update_with_elapsed` uses the same numerical implementation as `update`. Only the elapsed interval is injected. Ordinary runtime `update` retains its original clock-read location and advancement. The injected path neither reads nor advances that ambient clock. A regression checks its existing leak recurrence at zero, 500 ms, 2.37 s, 10 s and 100 s; no estimator threshold or leak coefficient was changed.

## Study Design

The final receipt is in the paired Minime worktree at `docs/steward-notes/2026-09-30-fill-coupled-qualification.json`.

- Backend: production Metal block matrix-vector multiplication, production Gram-Schmidt and Rayleigh quotient, production fill estimator, production rate tracker and shared structural controller/covariance/lifecycle.
- Matrix dimension 512, eight measured modes, fixed seed `805904422`, 48 steps per run. These are covariance measurement dimensions, not a simulation of the 128-node ESN.
- Eight fixtures: cold start, synthetic measured-history restoration, rank-one initial matrix, skipped regulation, unavailable observations, corrupt matrix, synthetic mid-run clock/controller restart, and a semantic-active interval.
- Three schedules: constant 500 ms; constant 2,370 ms; repeating 500/2,370/3,000/50/10,000 ms gaps.
- Two policies per fixture/schedule: observed-interval derivatives versus nominal-500-ms derivatives, both with the same repaired availability policy. This isolates the denominator contrast; it is not a byte-for-byte old-live-engine comparison.
- The fill estimator receives the same actual schedule on both sides. Its measured output drives subsequent controller decisions; fill trajectories are not imposed or replayed as a scalar plant.
- Per-step receipts include matrix hash, measured eigenvalues, matrix trace, estimator EMA, measured and control-input fill/rate, regulation rate, stage, mode, drain, recovery/reentry, reset generation, scaffold, settling and intake decision reason.

The scaffold is synthetic identity, not a captured live scaffold. Inputs begin at the projected-vector boundary and are supplied freshly each step. The harness does not reproduce persistence of the last sensory vector during silence. Intake decisions are recorded but are not applied to an upstream ESN or router. Pressure preference is held at zero. The semantic fixture changes the lifecycle flag, not semantic embedding/transport. The restart fixture restores a synthetic estimator snapshot and selected controller clocks; it is not a process restart or live-checkpoint compatibility test. The separate current-runtime controller profile is not qualified by this stable-core study.

Finite dimensions and explicit cooperative deadlines bound the executable. It accepts no paths, commands or endpoints. The final invocation also uses an inherited 240-second process alarm, OS network denial and OS file-write denial. The parent shell opens the single receipt output before sandbox entry. Reads are not OS-confined; the harness has no file-reading input interface and uses compiled synthetic fixtures. This is not represented as a general untrusted-worker sandbox or a memory-limited study service.

## Results

All 24 paired comparisons completed: 48 runs and 2,304 recorded feedback steps, plus a separate basis-reset probe. All eight constant-500-ms negative controls match exactly, including complete rows and covariance hashes.

| Scenario | 2.37-second changed action / matrix steps | Mixed-gap changed action / matrix steps |
| --- | --- | --- |
| Cold | 1 / 9 | 0 / 0 |
| Restored | 1 / 3 | 1 / 2 |
| LowRank | 0 / 0 | 0 / 0 |
| SkippedRegulation | 1 / 3 | 1 / 2 |
| InvalidObservation | 0 / 0 | 1 / 2 |
| MatrixFault | 1 / 3 | 1 / 2 |
| Restart | 1 / 3 | 1 / 2 |
| Semantic | 1 / 3 | 1 / 2 |

An action difference here means a changed operational mode or applied drain weight, not every possible internal state difference.

Two concrete comparisons:

1. Restored, mixed gaps, tick 10: control fill 43.4248047%, observed slope -0.7904747 pp/s versus nominal -15.809494 pp/s. The nominal clock enters recovery impulse one step earlier; the observed clock remains in scaffold hold at this step. The next step has recovery in both cases, but different matrix hashes remain for two steps.
2. Restored, 2.37-second cadence, tick 45: control fill 71.7657471%, observed slope +0.9059793 pp/s versus nominal +4.2943420 pp/s. Observed timing remains in scaffold reentry while nominal timing has moved to scaffold hold. Covariance hashes differ for three steps.

**Every paired reported-fill trajectory is identical in this corpus, not merely its final value.** Matrix differences must not be summarized as an observed improvement in fill stability. Conversely, an unchanged thresholded/smoothed fill statistic does not establish identical covariance dynamics. The full rows preserve both observations.

Restored histories also have high-fill excursions: for example the 500-ms fixture reaches 96.2258%, with two samples at or above 90% and an applied maximum drain of 0.70. These are retained results, not removed warmup samples. No claim of universal stability follows from all values being finite and bounded.

Unavailable observations at ticks 20-22 carry no measured rate through the first valid re-priming sample at tick 23. Corrupt-matrix reset at tick 20 also clears the rate. Restart, deterministic replay and the earlier finite-rate/missing-rate policy regressions pass. There is no inferred settling or experiential success from a test exit code.

## Newly Reproduced Measurement Defect

The low-rank fixture stays in recovery for 47 of 48 steps under both clocks. At 2.37-second cadence it ends at reported fill 0.0384317% after 113.76 simulated seconds. The recorded matrix trace is approximately 512 after reset, but seven Rayleigh estimates remain zero. The estimator EMA also retains earlier high-rank-one scale history (53.9125 at tick 0, 5.03039 at tick 47); its contribution is a separate question, not assumed to be the whole cause.

A direct identity-matrix oracle isolates the basis issue without the estimator or controller:

1. Evolve the production measurement operator on an exactly rank-one matrix for five steps.
2. Replace the matrix with production identity reset, retaining its existing power-iteration basis.
3. The production operator returns `[1, 0, 0, 0, 0, 0, 0, 0]`.
4. Reuse the same identity matrix with a fresh orthonormal basis: it returns `[1, 1, 1, 1, 1, 1, 1, 1]`, the independently known identity spectrum.

The existing Gram-Schmidt function divides a zero column by a small positive normalization floor; it does not replace that direction. Subsequent matrix multiplication cannot recreate a vector from zero. Runtime reset paths replace covariance but do not refresh the retained power-iteration basis. This gives a concrete measurement-recovery defect in a reachable numerical configuration, rather than merely a delayed controller response. It is reproduced at dimensions 32 (test) and 512 (receipt). The test intentionally characterizes the unresolved failure; it is not a claim that basis recovery has been repaired.

No evidence in this pass establishes that the current live engine has encountered this collapse, or that it caused Astrid's account. Do not automatically reseed the running basis or reset the estimator. A reviewed repair should preserve valid directions, replenish only deficient directions, expose measurement validity, and test estimator/reset semantics separately before returning to the coupled qualification.

## Verification and Attempts

Final suites: **817 tests pass**, zero failed/ignored in the selected suites: 418 library, 372 engine-binary unit tests, six coupled-harness tests, two timing-replay tests and 19 controller-review tests. Some library/binary modules share source; these are test executions, not 817 independent behavioral claims. The retained log is `2026-09-30-fill-coupled-tests.txt` in Minime's steward notes.

```sh
CARGO_TARGET_DIR=/Users/v/other/worktrees/afterimage-timing-20260930/target-minime \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --manifest-path minime/Cargo.toml --locked --offline \
  --lib --bin minime --bin fill_timing_replay --bin fill_coupled_qualification \
  --test fill_timing_controller_review -- --test-threads=1 --quiet

CARGO_TARGET_DIR=/Users/v/other/worktrees/afterimage-timing-20260930/target-minime \
CARGO_PROFILE_DEV_DEBUG=0 cargo build --manifest-path minime/Cargo.toml \
  --locked --offline --config 'profile.dev.package.minime.opt-level=2' \
  --bin fill_coupled_qualification

/usr/bin/perl -e 'alarm 240; exec @ARGV' /usr/bin/sandbox-exec \
  -p '(version 1)(allow default)(deny network*)(deny file-write*)' \
  /Users/v/other/worktrees/afterimage-timing-20260930/target-minime/debug/fill_coupled_qualification \
  > docs/steward-notes/2026-09-30-fill-coupled-qualification.json
```

Commands run from the Minime candidate. No `minime` engine executable is launched. Device: Apple M4 Pro, aarch64, macOS 27.0.1 (26A434), rustc 1.94.1. The final harness uses package-specific dev optimization level 2; tests use the normal test profile. Formatting and whitespace checks pass.

Preserved attempts and remaining limits:

- The initial harness compilation failed because its seeded RNG binding lacked `mut`; corrected before any successful run.
- Pilot 1 retains 16 comparisons (500-ms and mixed schedules), before later receipt fields and the fixed-delay case. Pilot 2 retains 24 comparisons before final receipt corrections and the basis probe. Their embedded source identities identify those historical builds, not final source. Neither pilot is silently replaced by the final JSON.
- Strict Clippy initially reported 75 library diagnostics, including the moved rank-one range-loop warning. The new warning was corrected; the final attempt still fails on the 74 previously recorded library diagnostics, before complete binary/test lint qualification. No blanket allow or baseline relaxation was added. The final harness-only basis probe was compiled/tested after this lint attempt; it has no separate clean strict-lint claim.
- Sandbox write probe (`touch` an owned temporary probe path) was denied. The first network probe used `/usr/bin/python3`, whose launcher failed before testing a socket because it attempted a denied write; it is not network-isolation evidence. The explicit `/opt/homebrew/bin/python3` retry was denied on loopback bind, confirming network denial. No probe file or listener was created.
- Whole-engine database/checkpoint, process restart, source reconciliation and immutable release qualification were not run. Bridge/Python/controller/projector source did not change; their historical results are not presented as new qualification here.

## Identities

The final JSON embeds and matches eleven source hashes, including covariance/controller/timing code, GPU/shader, estimator, lockfile and the harness itself. Additional runtime integration hashes and artifact identities:

| Artifact | SHA-256 |
| --- | --- |
| `minime/src/covariance_math.rs` | `c1f44b6535a0f81c940d523833c4ae352a95f025567fa75a382848207d526568` |
| `minime/src/stable_covariance.rs` | `0ee676503ae22b468617067788ea9bb58a4b3c31459cd6c1ac00ca627edfb54b` |
| `minime/src/bin/fill_coupled_qualification.rs` | `368c49876c8db4316958b7c9a770112b4d1a81e55a59932a785982bac9fc59bd` |
| `minime/src/runtime/orchestration.rs` | `21e8da66659a9912045cd646821dfbf40ea9099f0e59af386e4257e4cf70a945` |
| `minime/src/runtime/telemetry_broadcast.rs` | `85b49328f53912545c62dfd6059f1bde3eed08417844eaf9609b4b870671b80b` |
| `minime/src/runtime/spectral_math.rs` | `305aa191339d8bcce73c5f709a6b29a750e1bc6be62f06ae7c6ada2f76189b37` |
| `minime/src/spectral/eigenfill.rs` | `9ce31d2ed2871b5e6ed62b212e94e8aa38125fe546b553e463f890a035bec1a1` |
| `minime/Cargo.lock` | `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b` |
| final harness executable | `465241263313a468c89f6d9d63b6d2d5c82eff652ba08e8d23cc4fa4bc866146` |
| final qualification JSON | `f68727226e2d8fbba9e9479a6c4859bc40cc7fe080de5588beb8be5b0c8c2da3` |
| pilot 1 JSON | `4e91d32af7c360bebcf9f45fb234f7e724eedc746859e27e3f74f8bd61ac041f` |
| pilot 2 JSON | `847906829c76f04d5c4e10fc63ca5f42857c2bdbe5f7dfb18bb3d8a1648cc2a0` |
| final test log | `3352cb70fa0ea43f5c35b0a89c8cdd97ea619e65384c2a4893617de3edc9235e` |
| final strict-Clippy log | `eee10656a3c865214b025f6ef11e063106d37a839a7f7c50449acfec49985503` |

All JSON/log paths above are in the paired Minime `docs/steward-notes` directory. The qualification executable is in the isolated target directory shown in the commands; it is not the live engine artifact.

## Next Release Gates and Ownership

1. Repair and independently qualify rank-deficient measurement-basis recovery offline; test full/near rank deficiency, preserved valid directions, orthogonality, determinism and covariance reset. Separately characterize the retained estimator history. Do not use a broad estimator reset as an unreviewed substitute.
2. Rerun this coupled corpus with matched controls, extend coverage around the new numerical boundary, and inspect saturation/settling rather than equating finite outputs with stability. Other nominal-tick dwell/integral timers remain explicit separate debt.
3. Resolve or explicitly review the pre-existing strict-lint debt; reconcile only the owned engine delta with then-current main. Qualify an exact immutable release, launch inputs, checkpoint compatibility, rollback and sanctioned transition under separate engine-rollout approval.

Both canonical repositories were rechecked clean on `main`, each two commits ahead of its locally recorded `origin/main`; no remote fetch/push was performed. No staging, commits, merges, resets or cleanup occurred. Earlier dirty work remains in the isolated owned candidates. The controller's read-only start status remained paused at generation 484, without a lease or projection. It reported V2 indexed-tail valid at sequence 1123138, head `98e9ec873793dc0e619017d687a252f157ef13a57859231a73d2401cc154ba57`, V1 immutable. These are status observations, not newly appended event-store evidence. No introspection round or automation resume was recorded.

The implementation and this study address trustworthy numerical evidence. They establish no subjective improvement, consent, uptake or authority to alter live reservoir behavior. No request for confirmation was sent to either Being.
