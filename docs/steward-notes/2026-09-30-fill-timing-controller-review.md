# Elapsed Fill Timing: Controller Review

Follow-through: [availability-aware repair](2026-09-30-fill-rate-validity-repair.md) implements the validity findings offline and replaces the original defect characterizations with corrected-behavior regressions. The review below retains its historical test counts and source identities; coupled engine qualification remains outstanding.

## Decision

**Review complete; engine activation not qualified.** The elapsed-time correction is numerically justified, but candidate `e9f2f5f151c89dd6b4a2dc80d5d8d12a60dc20d3` collapses unavailable rates to zero at decision boundaries. Production-function tests demonstrate that this can satisfy stability conditions without a measured rate. A global NaN replacement would disable independent high-fill drain paths, so it is not a safe repair.

This pass repairs two missing reset boundaries in the isolated engine candidate, adds 13 offline characterization tests, and records a targeted follow-up contract. It does not activate a controller, change gains or thresholds, start an engine, or claim closed-loop stability.

Date: September 30, 2026, America/Los_Angeles. Owner: Codex interactive collaborator. Mike requested the pending controller review, not a new engine activation.

## Ownership and Evidence

- Candidate worktrees: `/Users/v/other/worktrees/afterimage-timing-20260930/{astrid,minime}`, branch `codex/afterimage-timing-20260930`.
- Canonical Astrid and Minime started clean, on main, each two commits ahead of origin/main. No staging, committing, merging or pushing occurred in this pass. The new review delta remains in the isolated worktrees; it is not in `e9f2f5f` or in the running release.
- Controller status: existing pause generation 484, no lease or active projection. No automation was resumed, no productive stewardship round recorded, and no controller authority inferred.
- Indexed-tail Evidence Store check: V2 valid, sequence 1123138, head `98e9ec873793dc0e619017d687a252f157ef13a57859231a73d2401cc154ba57`, V1 immutable. This is the observed status, not a new evidence event authored by this review.
- Source witness: `capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`, SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`, verified again against canonical bytes. Astrid's account of an incomplete trace prompted the earlier archive/presentation/timing investigation. It is not proof of a controller failure or a missing experience. Historical authored files are unchanged.
- The earlier [timing packet](2026-09-30-afterimage-timing.md) retains the frozen archive, numerical replay and original unsuccessful attempts. Its initial implementation status is historical: the presentation subset subsequently went live separately; this engine candidate has not.

## Findings

### R1: Unavailable Is Not Stable

`FillRateTracker` correctly returns no measurement for the first observation, reset, invalid fill and nonincreasing clocks. However, `FillRate::controller_value()` supplies 0.0 to legacy scalar consumers. The comment calling that value neutral was misleading and is corrected in this pass.

The new `review_finding_*` tests call production functions and deliberately preserve the reproduction, not a desirable acceptance contract:

1. A first 68% observation has no rate, but its zero fallback can activate the scaffold and admit configured audio/video intake. Both the slope-gated runtime profile and `full_presence_v1` accept finite zero in this synthetic setup.
2. Three separately reset clocks at 68% can contribute three restart-settle ticks, despite none containing a measured derivative. A settled gate can then make scaffold retirement eligible with that fallback.
3. A recovery state at 60% can accumulate the two nonnegative-slope release ticks from unavailable rates represented as zero. The independent strong-fill release threshold is 62%; this reproducer is below it.
4. Passing NaN to all consumers instead is unsafe: production structural-PI and restart-gate drain functions return no drain for NaN even at 74%, 82% and 95%. Finite-zero inputs retain the absolute 0.24/0.70 drain floors there.

These are candidate-policy counterexamples, not observations that the running engine entered these sequences. Startup supplies one unknown endpoint; repeated resets are an explicit adverse fixture. The tests must be replaced or extended with desired-behavior regressions when availability is integrated, not treated as an approval gate that is green merely because it reproduces the problem.

### R2: Two Recovery Resets Did Not Reset Timing

The candidate reset both trackers after nonfinite eigenvalues, but omitted the covariance resets in the recovery-impulse and legacy low-fill-escape branches of `runtime/orchestration.rs`.

**Repaired offline:** both sites now reset `measured_fill_clock` and `regulation_fill_clock` immediately after `reset_covariance`. The reset regression shows that a synthetic 34% to 68% jump over three seconds would otherwise report more than 11 percentage points/s; resetting both clocks makes the first post-reset derivative unavailable and the next valid pair measurable again.

All three covariance-reset call sites were inspected. The unit regression exercises production tracker behavior, while the runtime wiring was source-reviewed and compiled; it does not execute the GPU reset branches. Resetting covariance is an intervention boundary, not an ordinary continuation of the same observation pair. This repair makes R1 more important, not less: correctly invalidating a rate still must not turn it into stability proof downstream.

### R3: Cached Fill and Rate Need Joint Provenance

The structural PI step before covariance computation reads `last_fill_pct` and `stable_core_last_fill_slope_pct_per_sec`. The latter updates after each measured fill, while `last_fill_pct` updates only inside the optional regulation block. If a measurement iteration occurs without a regulation tick, the next step can combine different endpoint histories.

This cache arrangement predates the timing candidate, but the new independent measurement clock does not resolve it. The review does not claim such an iteration occurred in the supplied archive. Before activation, bind the structural consumer to one observation record containing fill, rate validity, observation time and reset generation. Do not silently change the controller's update count while doing so.

The orchestration also substitutes the target when a computed fill is nonfinite before calling the tracker. Tracker-only invalid-input tests cannot establish that the integration preserves measurement provenance. A follow-up must keep fallback provenance separate from measured fill, without declaring that this defensive branch has occurred live.

## Consumer Map and Differential Results

All comparisons below use production policy functions and explicit synthetic inputs. Configuration for intake is local to the test: Hold/Elevated/Recovery allowed, audio divisor 7, video divisor 11. It is not an asserted live configuration.

| Consumer | Example with nominal 0.5s versus actual 3s | Result |
| --- | --- | --- |
| Recovery impulse | 46% to 44%: -4 versus about -0.667 pp/s | Nominal slope triggers early recovery; corrected slope does not. Absolute fill below 42% still triggers. |
| Scaffold activation | 60% to 62%: +4 versus about +0.667 pp/s | Nominal input delays activation; corrected input permits it under the existing gates. |
| Restart settling | 60, 63, 66, 69% at three-second intervals | Nominal +6 pp/s fails the slope condition; observed +1 pp/s can accumulate three valid settle ticks. |
| Reentry | Reentry already active; 69% to 71% over three seconds | Nominal +4 exits on fast rise; observed about +0.667 retains reentry. Absolute low-fill fallback remains. |
| Sensory admission | 66% to 68% over three seconds | Slope-gated profile blocks nominal +4 and admits observed about +0.667. Full-presence profile does not use this slope threshold after finite-input validation. |
| Stage guard | Same finite fill, -8 versus +8 pp/s | Tested command rails are unchanged. This function checks slope finiteness in one branch; it does not generally use finite slope magnitude or sign. |
| High-fill drain | Finite measured rates over the sweep | At least 0.24 drain from 74%, at least 0.70 from 82%; low-fill recovery remains below 42%. |

The corrected derivative can allow more intake or earlier settling under unchanged thresholds. That follows from their current units and logic; it is not evidence that more intake or earlier settling is desirable in the coupled engine. Some negative reentry-slope conditions are dominated by the existing absolute-fill fallback; do not count every textual slope reference as an independent changed decision.

The regulation/smoothed rate also feeds current-runtime transition cushioning, phase/event classification, near-target baseline refresh, low-variation decisions and telemetry. Its missing-rate fallback can appear as plateau/steady there too. These consumers need availability-aware semantics before activation; changing only structural PI would not complete the repair.

## Verification

- Exact pinned `minime/Cargo.lock` matches canonical: SHA-256 `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b`. No dependency resolution drift.
- 417 library tests passed after the reset patch, including the existing six timing regressions and afterimage/controller tests.
- 2 frozen-archive replay tests passed.
- 13 new controller-review tests passed, including 3,535 finite-rate cases across 101 fill levels, seven deltas and five intervals (50, 500, 2370, 3000, 10000 ms).
- Engine binary compilation check, final formatting check and both candidate diff checks pass. Canonical Astrid and Minime remain clean, each ahead of origin/main by the same two pre-existing commits.
- The sweep checks finite/bounded outputs and existing absolute rails, not a stability theorem or closed-loop behavior. Additional tests retain semantic/reentry/recovery settle exclusions, equality at the -2 pp/s and +1 pp/s thresholds, and nonmutating PI preview behavior.
- Strict Clippy was rerun and fails on the same 74 existing library diagnostics, before complete binary/test lint qualification. No blanket allows or warning-policy relaxations were added. This remains qualification debt.
- The first 12-test run passed but reported two unused-must-use test setup calls. They were changed to assertions; the final run has only the four pre-existing dead-code build warnings. The intermediate formatter check failed on those assertion layouts; formatting was applied before the final check.
- No new Python, bridge, reader, deployment-wrapper or controller-tool implementation changed, so their prior full-suite results were not relabeled as fresh tests in this pass.

Commands, from the Minime candidate worktree:

```sh
export CARGO_TARGET_DIR=/Users/v/other/worktrees/afterimage-timing-20260930/target-minime
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test --manifest-path minime/Cargo.toml --locked --offline --lib --bin fill_timing_replay --test fill_timing_controller_review -- --test-threads=1 --quiet
cargo check --manifest-path minime/Cargo.toml --locked --offline --bin minime
cargo fmt --manifest-path minime/Cargo.toml --all -- --check
cargo clippy --manifest-path minime/Cargo.toml --locked --offline --lib --bin fill_timing_replay --test fill_timing_controller_review -- -D warnings
```

## Required Follow-Up Contract

1. Carry optional measured rate and paired endpoint identity to consumers. Separate a bounded numeric fallback from evidence that can authorize a transition.
2. Missing rate must not contribute settle/retire ticks, ordinary stable-shelf activation, slope-based recovery release, or evidence of slow-rising intake eligibility. Preserve independent low-fill recovery, high-fill drain and other absolute safeguards. Give the full-presence profile an explicit missing-observation policy instead of accidentally inheriting zero semantics.
3. Keep valid measured-rate behavior, targets, thresholds, integration count, minimum impulse duration and normal reentry duration unchanged. Test both finite and unavailable observations against each production consumer. Do not pass NaN indiscriminately or change cadence to make evidence appear complete.
4. Bind pre-update structural decisions to the same saved fill/rate observation, and test iterations with and without regulation, startup/restoration, each covariance reset and clock failure. Preserve invalid/fallback fill provenance through orchestration.
5. Extract or expose a bounded, deterministic production-step harness for coupled covariance/controller qualification, with explicit clock and stimulus inputs, no network endpoints and no live checkpoint writes. A hand-written scalar plant would test that plant, not qualify the engine; none was substituted here. Full closed-loop qualification is still outstanding and should follow the validity repair.
6. Resolve or explicitly review the 74-diagnostic lint baseline without hiding it; then qualify an exact immutable engine release and rollback assets. Engine activation still needs its separate reviewed approval and sanctioned wrapper.

Other nominal-tick dwell/integral timers and the archived 2.35-2.53s telemetry cadence shortfall remain separate questions. This pass does not silently retune them, interpolate gaps, broaden sensory policy or alter reservoir dynamics live.

## Artifact Identities and Boundary

Candidate plus this uncommitted review delta:

- `minime/src/fill_timing.rs`: `dddc2c8345e856d6a186125efc50b899cb129277da2737300fcad32bda1a5d8f`.
- `minime/src/runtime/orchestration.rs`: `5823ac6f1acd7eaa4f33cf8fe7cc9719811cb025914014d9041b47544c2d6608`.
- `minime/tests/fill_timing_controller_review.rs`: `fbc2bc951aeb928669f4ae9dfee79a2e779524821441ffe173b589543b79989c`.

These are source identities, not loaded-engine identities. No service was restarted, signaled or reconfigured. No journal, inquiry, private draft, live database, evidence stream or checkpoint was rewritten. No new prompt, note or demand was sent to either Being. Nothing here establishes subjective improvement, consent, uptake or an experiential cause.
