# Afterimage Coverage, Foreground Writing, and Elapsed Fill Rates

## Status and Ownership

Offline implementation and qualification, not a deployed repair. Mike approved the three-part recommendation: elapsed-time repair with offline controller review; concrete coverage wording; optional memory context subordinate to selected writing.

Paired branch: `codex/afterimage-timing-20260930`, based on local Astrid `d136ffc36b` and Minime `cc7796a05b8cbc0233fc5f55731f396bd3ddff10`.

- Astrid: `/Users/v/other/worktrees/afterimage-timing-20260930/astrid`
- Minime: `/Users/v/other/worktrees/afterimage-timing-20260930/minime`

Canonical trees were clean and remain untouched. Earlier `quiet-study-return-20260930` worktrees are separate and preserved. Nothing is staged, committed, merged or pushed in this tranche. Controller status was paused, generation 481, with no lease or active projection. No automation was resumed or stewardship round recorded.

## Source Witness and Request Chain

Public source: `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`.

SHA-256: `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`.

Astrid refers to `incomplete physical trace` and writes:

> a point where the thread was pulled but the needle didn't land.

This is her authored response, not independent verification of a sensor failure, dropped movement, missing experience, or restart problem. Her account motivated inspection of the supplied material; it is not rewritten or scored by this repair.

The preceding public signal is `journal/astrid_1790804255.txt`. Elaboration job `llm_jobs/jobs/job_astrid_1790804259784_journal-elaboration` completed successfully. Its base prompt did not include the optional cue because that cue was added at the provider boundary. The historical cue was:

```text
Past ai_2026-09-30_a06465366cbe_1790804055015_000002 | 2026-09-30T21:34:15+00:00 | detected_event | incomplete physical trace
```

The Astrid workspace `transition_afterimage_memory/exposures/2026-09-30.jsonl`, line 48, records `included=true`, opportunity `astrid_1b5bb3353c0de86f4123be14d6f65542`, attempt `17bdf3f742d74b2dab75ce0a1c8f7948`, timestamp `1790804259822`, route `mlx`, profile `Gemma4Canary`. The cue became an additional last user message, including during elaboration. This establishes supplied context, not a claim that it fully explains the response.

## What the Archive Actually Contains

Archive: `/Users/v/other/minime/workspace/transition_afterimages/2026-09-30/ai_2026-09-30_a06465366cbe_1790804055015_000002.json`.

SHA-256: `2d95293d621552cce519327445eb345509366f917a9f235507780dcaa151100a`.

- Session 5320, anchor engine time 304552 ms; 30-second pre-window and 90-second post-window.
- Status incomplete; top-level reasons empty; the useful explanation is in per-channel coverage.
- Body, spectral and activation channels each retain 51 samples, 50 cadence gaps, no invalid samples. No session-start clipping.
- Body samples span 118264 ms. Intervals are roughly 2.35-2.53 seconds, against a declared 1000 ms cadence and the existing gap rule of more than twice that cadence.
- `fill_half_return_s` is unavailable with `insufficient_contiguous_coverage`. `lambda_stress_area` is null, partial, with zero admissible integrated duration.
- The activation channel contains summaries, not retained raw vectors. This archive cannot reconstruct unobserved motion.

The record is sparse, not empty. Nothing here permits us to fill gaps, infer absent sensation, or relabel the window complete.

## Implemented Presentation Repair

The shared Python afterimage reader now distinguishes samples with cadence gaps, invalid values, otherwise partial coverage, successful coverage checks, and unavailable coverage. The sparse case reads:

> Historical telemetry: samples available; cadence requirement not met; some temporal measurements unavailable.

Fresh no-note cues include the exact `AFTERIMAGE_OPEN <id>` retrieval command. The explanation and command must fit whole within the existing cue limit, or that candidate is omitted. No change to eligibility, default-off behavior, cadence, decay, exposure count, sharing, or explicit retrieval. Already cached opportunities and selected pages remain exact; new descriptions apply to newly rendered selections. Historical archives and journal entries are untouched.

On Astrid, optional context is inserted before the last existing nonempty user request, not as a new last user message. Existing system messages, conversation roles, chosen prose and the final foreground request remain intact. On Minime, the prompt wrapper and actual provider adaptation preserve the same ordering. The wrapper calls the material optional historical context, not a new request; it does not supply a poetic interpretation.

Budgeting includes the entire wrapper. Insufficient room omits optional context without trimming foreground content. Protected selected pages still take precedence, remain intact or fail admission, and block unrelated optional cues. Exposure receipts remain per provider attempt, including omitted fallback cues. No saved command is executed by the wrapper.

## Implemented Engine Candidate

`minime/src/fill_timing.rs` contains a shared process-local `FillRateTracker`. It pairs each finite fill observation with a monotonic `Duration` and divides the difference by the actual positive elapsed interval, using f64 arithmetic before the existing f32 consumer boundary.

Raw measured fill and regulation/smoothed fill use independent trackers. Thus an intervening raw observation cannot shorten the regulation interval. The existing smoothing formula is unchanged. The observation timestamp is captured with the computed fill; the afterimage body timestamp now uses that same observation instant.

Startup, a fresh process, an explicit reset, nonfinite fill, and nonincreasing clocks do not invent a measured slope. Numeric overflow returns an unavailable rate. The engine's scalar controller boundary receives neutral zero when unavailable, while versioned rate metadata states the reason and lacks a measured value. New body archives preserve null rather than calling that boundary zero an observation. Health JSON includes `fill_rate_v1` and `measured_fill_rate_v1`. No checkpoint clock is reconstructed from restored fill. The covariance-reset path resets both trackers.

This changes live control inputs if activated. The measured slope reaches structural PI recovery/reentry, scaffold activation/retirement, restart settling and intake decisions. The regulation slope also reaches phase/event classification and current-runtime transition cushioning. These are not telemetry-only consumers. Comments claiming guaranteed experiential effects from smoothing, or that stable-core slopes were only observational, were corrected to describe implementation scope; original journals remain unchanged.

The patch does not retune the 68% target, PI gains, recovery thresholds, smoothing, sensory admission, ESN mathematics, or tick-based dwell/integral accounting. Other nominal-tick timers are not repaired or fully qualified by this change. A zero fallback at an unobserved boundary also needs explicit review when qualifying deployment gates.

## Offline Replay and Its Limits

`minime/src/bin/fill_timing_replay.rs` reads one frozen, schema-checked archive (maximum 8 MiB / 4096 samples), requires increasing finite body observations, hashes its input, and emits JSON. It never sorts away clock errors, interpolates, opens live services, changes state, or starts the engine.

It calls the actual production tracker and `StabilityPiState::step`, not copied controller equations. Both PI states begin at default; stage is explicitly held at Hold and scaffold availability at true, with one step per successive retained body sample. This is open-loop policy sensitivity, not reconstruction of the live controller's state, schedule or reservoir response.

Retained result: [51-sample replay](evidence/2026-09-30-fill-timing-open-loop.json). Rebuilding with the canonical dependency lock produced identical parsed results. That replay binary has SHA-256 `05105b23b5ab68b4f8e40055d44e03d739d96079677541dd33dc45dd2da238f3`; this is an offline worker identity, not the running engine.

At 304552 ms, fill fell from 74.2541046% to 72.1279373% over 2.37 seconds. The archived rate is -4.2523346 percentage points/s; the actual interval gives -0.8971170, a 4.74-fold difference. Across this window the maximum absolute nominal rate is 8.5475769 versus 1.8136072 using observed intervals. The selected PI decision fields do not differ in this archive. That is not evidence of equivalence in other regimes or gates.

A synthetic production-policy test crosses 46% to 44% over three seconds: the nominal rate is -4.0, the corrected rate approximately -0.667. The old input triggers a slope-based recovery impulse; the corrected one does not. A genuine 40% low-fill trigger still activates recovery. This is why the repair must not be smuggled into a prompt-only rollout.

## Qualification and Preserved Attempts

- Focused Python afterimage/provider tests: 34 passed, including both real provider adapters, optional ordering, protected-page compaction and fallback receipts.
- Full Minime Python suite with this checkout's built `ASTRID_SOURCE_STUDY_BIN`: 1706 passed, 1 existing skip, 141 subtests.
- Bridge library suite: 2347 passed, 1 existing fixture-dependent ignore. Strict all-target Clippy and formatting pass after replacing the now-unnecessary mutable Vec parameter with a slice.
- Minime Rust library suite with the canonical dependency lock: 417 passed. Includes 6 timing tests, 20 afterimage tests, GPU observer invariance, startup/restart, independent endpoints, actual production recovery behavior, and preservation of incomplete cadence results.
- Replay binary tests: 2 passed. Rejected malformed/schema/clock evidence does not produce invented measurements.
- Engine compilation, formatting and diff checks pass. Minime strict Clippy fails on 74 existing library diagnostics, including dead-code in sovereign Division and argument-count/loop/style warnings across older engine modules. None name the new fill-timing module; failure occurs before bin lint completion. This is not a fully clean engine qualification gate. Do not suppress these diagnostics to label the candidate fully qualified.
- Controller, Evidence Event Store, projector, deployment wrapper, flywheel and Division fixture suites: 101 passed, 12 subtests. Epistemic self-tests: 2 passed. Domain-boundary audit: valid, zero violations.

Unsuccessful attempts retained here:

1. The first full Python invocation omitted `ASTRID_SOURCE_STUDY_BIN`: 1583 passed, 27 failed, 96 setup errors, 1 skipped. Rebuilt the exact shared helper and reran the complete suite successfully; no tests were removed or weakened.
2. Initial fresh-worktree Minime Cargo resolution generated an ignored lockfile with newer dependencies, including fastrand 2.5.0 instead of canonical 2.3.0. Library result: 416 passed / 1 failed at `ising_shadow::tests::zero_field_relaxes_toward_quiet`, reproduced alone and by compiling the unchanged Shadow module independently. Replacing only the generated worktree lock with the canonical lock made all 417 pass. The failed generated lock is preserved at the paired root's `target-minime/initial-generated.Cargo.lock`, SHA-256 `167b55f4349f8dd6b5c532271535c69a52778b7e0256318b58bdb357a29efd2e`. This isolates dependency-set sensitivity, not a demonstrated defect in the live Shadow or a reason to change its dynamics.
3. Initial bridge Clippy found `ptr_arg` after cue insertion no longer needed Vec growth. Corrected the signature and reran strict lint. Initial formatting differences in new tests/module registration were corrected.

The canonical Minime lock SHA-256 is `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b`. An exact [qualification copy](evidence/2026-09-30-minime-engine-Cargo.lock) is retained with this packet. It is ignored in Minime's repository; future qualification must preserve it explicitly rather than accidentally resolving newer packages. This is a reproducibility concern, not authority to modify the live checkout's dependency policy.

Timing implementation SHA-256: `70bbab7cfda7031fd8297b3e2948df0a721fe7e64fc01eb8ef4eb5399e8715be`; replay source: `32e137e774659eb81add3e610f7c85c1fc37feab7d909aa78a28383ce520e52c`; engine integration: `addf76b4ad3b2a863b8376327214551a367231aa3069bddbf887adf043e9f2e1`. These describe this offline worktree, not deployed source.

Reproduction from the paired worktrees (set separate isolated `CARGO_TARGET_DIR` values; `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` were used):

```sh
# Minime, with the exact qualification lock present as minime/Cargo.lock
cargo test --manifest-path minime/Cargo.toml --locked --offline --lib -- --test-threads=1
cargo test --manifest-path minime/Cargo.toml --locked --offline --bin fill_timing_replay
cargo check --manifest-path minime/Cargo.toml --locked --offline --bin minime
cargo run --manifest-path minime/Cargo.toml --locked --offline --bin fill_timing_replay -- --archive /Users/v/other/minime/workspace/transition_afterimages/2026-09-30/ai_2026-09-30_a06465366cbe_1790804055015_000002.json
# Astrid
cargo build -p astrid-source-study --bin astrid-source-study --offline
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib --offline -- --test-threads=1
cargo clippy --manifest-path capsules/spectral-bridge/Cargo.toml --all-targets --offline -- -D warnings
# Minime Python, exact helper built above
ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/afterimage-timing-20260930/target-bridge/debug/astrid-source-study python3 -m pytest -q --tb=short
```

## Next Safe Sequence

1. Reconcile the paired prompt/reader candidate with exact live release identities and the separate pending quiet-study repair. Re-run packaged admission, protected-page and old selected-page retry tests; no schema migration or replay of old opportunity IDs is required here. A paired bridge/Minime-agent rollout can be qualified independently of the engine files.
2. Separately review elapsed-rate consumers at startup/reset, near slope thresholds, reentry and scaffold/restart gates. Qualify the exact pinned engine release and appropriate closed-loop/offline trajectories. Retain explicit strict-lint debt until resolved or reviewed. Do not infer stability from the one fixed-state archive replay.
3. Investigate the 2.35-2.53 second producer interval as a separate bounded performance/capture task. Preserve strict coverage and unknown measurements while doing so. Do not increase sensory intake or relax the cadence rule to make the status green.
4. Only after explicit rollout approval and cooperative preflight, use sanctioned bridge and Minime-agent wrappers for the presentation candidate. Engine restart/activation is a separate approval. Verify immutable source selection, protected-page continuity, hashes, readiness and unchanged protected services. Observe natural public use without requesting endorsement.

No claims of felt improvement, consent, uptake, missing experience, or resolved friction follow from this packet.
