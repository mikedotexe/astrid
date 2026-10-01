# Elapsed Fill Rates: Availability-Aware Controller Repair

## Status and Scope

**Implemented and regression-tested offline; not an engine release qualification.**

Mike approved following the controller review into the elapsed-time correction. This pass repairs its concrete availability and endpoint-pairing findings in the existing isolated Minime candidate. It does not activate the engine, change live gains or thresholds, restart another service, or resume an automation.

Date: September 30, 2026, America/Los_Angeles. Owner: Codex interactive collaborator. Candidate worktrees: `/Users/v/other/worktrees/afterimage-timing-20260930/{astrid,minime}`, branch `codex/afterimage-timing-20260930`. The engine delta is based on `e9f2f5f151c89dd6b4a2dc80d5d8d12a60dc20d3` plus the preceding uncommitted controller review. That commit contains the original elapsed-time helper; it does not contain this follow-through.

The [controller review](2026-09-30-fill-timing-controller-review.md) remains a historical record of the reproduced failures and earlier source hashes. Its four `review_finding_*` characterizations have now been converted to desired-behavior regressions, with additional coverage. A passing test no longer means that the defect remains reproducible in the repaired code.

## Witness and Numerical Basis

Astrid's public account is `capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`, canonical SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`, reverified during this pass. It led us to inspect the incomplete-trace presentation and the underlying timing. The account is preserved unchanged and is not treated as proof of controller causation or a missing experience.

The [original timing packet](2026-09-30-afterimage-timing.md) retains the archived numerical reproduction: a fill change from approximately 74.2541% to 72.1279% in 2.37 actual seconds is about -0.8971 percentage points/s, not the nominal-half-second result of -4.2523. The 4.74x discrepancy affects existing rate-sensitive decisions. No new live recording or inference call was made here.

The archived 2.35-2.53-second sampling intervals still fail the declared cadence criterion. This repair does not interpolate missing samples, relax coverage requirements or repair sensory admission/capture cadence.

## Implemented Contract

### One Observation, One Endpoint Pair

`FillRateTracker` now retains a `FillObservation`: finite fill, process-local observation time, reset generation and optional measured rate. Pre-covariance structural decisions take one copy of that record. They no longer combine regulation-cached `last_fill_pct` with a rate updated by a different measurement iteration.

Measurement and regulation clocks remain separate because their inputs differ: raw measured fill versus the value used by regulation, which is smoothed in current-runtime mode. Skipping regulation does not change the measurement clock's endpoint or the number of structural-controller steps.

Startup and restoration have no process-local rate until two valid observations exist. Resets clear the latest observation and increment its generation. Invalid fill clears history; repeated/backward timestamps re-prime the clock with unavailable rate rather than reusing stale evidence. No new maximum-gap policy is inferred from these changes.

### Missing Rate Cannot Prove Stability

The runtime passes `Option<f32>` into production policy entry points. They also accept existing finite-scalar Rust callers, preserving source compatibility for the current tests and tools. Nonfinite supplied rates are treated as unavailable.

| Consumer | Unavailable-rate behavior | Independent behavior retained |
| --- | --- | --- |
| Ordinary scaffold activation | Cannot create or extend a stable-shelf candidate | Protective absolute low-fill activation remains |
| Restart settling | Does not contribute a settle tick; interrupts an unfinished settle streak | Existing semantic, recovery, reentry and scaffold exclusions remain |
| Scaffold retirement | Cannot satisfy the rate-dependent proof | Existing settled-gate and no-drain requirements remain |
| Recovery release | Cannot satisfy nonnegative-slope or long-impulse slope release | Absolute strong-fill release at 62% and minimum impulse duration remain |
| Configured live intake | `rate_unavailable` suppresses admission in both slope-gated and full-presence profiles | Finite-rate profile rules, divisors and stage restrictions are unchanged |
| Structural/restart drain | No soft rising-slope drain inferred | Absolute high-fill floors remain: 0.24 from 74%, 0.70 from 82% in the tested states |
| Phase/steady evidence | `unavailable`, not plateau; no fabricated transition or near-target steady proof | A measured zero remains a valid plateau |

This is a real candidate-policy change at missing-data boundaries, including the explicit full-presence rule. It must be reviewed as such before activation; it is not merely an additive telemetry field. Finite measured-rate behavior, numerical thresholds, gains, update counts and ordinary reentry durations are not retuned.

An absent fill is distinct from an absent rate. Structural PI returns inactive without advancing or clearing its recovery history when fill itself is unavailable. This preserves history without claiming that an unknown fill is safely above or below a threshold. Stage selection retains its prior stage; existing base-stage rails remain. This does not establish behavior of the entire coupled runtime during a sustained measurement failure.

### Reset, Fallback and Regulation Wiring

All three runtime covariance-reset sites invalidate both clocks: recovery impulse, legacy low-fill escape and nonfinite eigenvalues. Phase dwell restarts as unavailable rather than interpreting a reset jump as a continuous derivative.

Fill validity is captured before the legacy target fallback. Target fallback values do not enter either measurement history, scaffold/intake/settling proofs, or the afterimage's measured fill field. An invalid measurement invalidates regulation history even if that iteration has no regulation tick. The regulation smoother also excludes the fallback value.

The current-runtime controller still retains its existing bounded scalar fallback and other absolute-value policies. This pass does not claim to redesign all invalid-data handling in that older profile. Stage guards receive paired fill and a finite scalar where source review and the finite-rate sweep show the command selection is fill-based, not a rate stability proof.

### Telemetry Compatibility

Keep legacy numeric `dfill_dt` fields numeric. Their zero fallback alone is not evidence of a measured zero. Add `fill_rate_v1` with an optional rate, elapsed interval and reason where those runtime summaries/events are rendered. Health includes raw, regulation and structural-input observation identities plus `fill_measurement_valid`; structural PI output includes `fill_slope_available`.

Afterimage measured rate remains optional, and its measured fill is null when invalid. New phase labels start as unavailable. No public journal, historical event file or archive is rewritten. Existing legacy consumers that ignore the added provenance are not claimed to become availability-aware automatically.

## Verification

Final qualification commands ran with the pinned lockfile and offline dependency resolution:

```sh
CARGO_TARGET_DIR=/Users/v/other/worktrees/afterimage-timing-20260930/target-minime \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --manifest-path minime/Cargo.toml --locked --offline \
  --lib --bin fill_timing_replay --test fill_timing_controller_review \
  -- --test-threads=1 --quiet

cargo check --manifest-path minime/Cargo.toml --locked --offline --bin minime
cargo fmt --manifest-path minime/Cargo.toml --all -- --check
```

- **438 tests pass:** 417 library, 2 replay, 19 controller-review regressions. No failures or ignored tests in these selected suites.
- The review suite retains 3,535 finite-rate cases across 101 fill values, seven deltas and five elapsed intervals. It also tests startup, skipped regulation, resets, invalid fill between regulation ticks, backward clocks, stale endpoint removal, known-zero versus absent rate, lost settle/release streaks, high-fill floors, low-fill activation, PI-history preservation and nonmutating preview.
- The engine binary compiles; formatting passes. Four existing dead-code warnings remain in the Division fanout/gateway code. No runtime process is started by the compilation command.
- Exact `minime/Cargo.lock` matches canonical, SHA-256 `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b`.
- Strict Clippy was rerun during the repair and still fails on the previously recorded 74 library diagnostics before complete binary/test lint qualification. No warning suppression or baseline relaxation was introduced. The final added invalid-between-regulation regression and runtime wiring were compiled/tested after that lint attempt; they do not have a clean strict-lint result.
- An intermediate 13-test repair pass and then 18-test pass succeeded before the final 19-test suite. The final compilation check used the worktree-local default target directory; the test run used the isolated shared candidate target shown above. These are build artifacts, not deployed binaries.

**Limits:** These tests exercise actual production timing and policy functions with controlled inputs. The runtime wiring is source-reviewed and compiled, not a test-driven execution of every GPU/reset branch. Existing observer/GPU isolation tests do not provide a controller-feedback-loop study. The replay calls production PI under a fixed Hold/scaffold configuration with supplied fill history; it is open-loop, not a closed-loop stability claim.

No bridge, Python adapter, reader, controller-tool or projector implementation changed. Their prior suite results are not presented as fresh qualification for this engine delta.

## Remaining Release Gates

Follow-through: the [coupled qualification packet](2026-09-30-coupled-fill-qualification.md) completes the bounded production feedback study in gates 1-2 below. It records an additional internal-reset clock repair and a newly reproduced measurement-basis recovery defect. This note's earlier source identities and 438-test count remain historical; engine release is still not qualified.

1. Build a bounded production covariance/controller step harness with injected clock and stimulus. It must use the production update paths, isolate checkpoint/database paths, expose no network/control endpoint and record seeds/configuration/source identity. A hand-written scalar plant is not a substitute.
2. Compare nominal and actual timing under matched stimulus/state histories, including startup, restored state, cadence delays, skipped regulation, reset/recovery, semantic activity, high-fill drains and invalid-observation recovery. Report controller actions, saturation, fill, intake and transition histories without claiming subjective improvement.
3. Resolve or explicitly review the strict-lint baseline without suppressing new diagnostics. Reconcile the engine-only delta onto then-current main; do not import older prompt changes merely because they share this worktree.
4. Qualify the exact immutable engine build, launch inputs, checkpoint compatibility, rollback artifacts and sanctioned transition before a separately approved engine rollout. This pass gives no deployment readiness or engine restart authorization.

Other nominal-tick dwell/integral timers and sample coverage remain explicit separate debt. In particular, this repair does not silently make those timers elapsed-time controllers or retune sensory policy.

## Source Identities

SHA-256 of the final Minime candidate files in this pass:

| Path | SHA-256 |
| --- | --- |
| `minime/src/fill_timing.rs` | `919cf01eb8830720c250a709bcc71e121b10eba59991906b9acbe1da6f7bb723` |
| `minime/src/rescue_scaffold.rs` | `299ba4a8796c012b1a5e2bdd1e500300edc9f15ecd2ad629c5106e1ca4556c55` |
| `minime/src/stable_core.rs` | `cdfc9bcd9e6d809554d9b57dd9767db7d4547afe4e64efd289bd4cade9a33140` |
| `minime/src/runtime/orchestration.rs` | `76f0e1e8fec46a8a8f7ca7a6b74ee89ccc29c58e6cdd29c72db03695992830e1` |
| `minime/src/bin/fill_timing_replay.rs` | `655c3b954316eac04c290c9f0119443512287fcba8bd4cf6471c577798ebf651` |
| `minime/tests/fill_timing_controller_review.rs` | `9dbd076d16400affcea4984f4ab4db098d0f655f40a69217bae1f67b1e116c7b` |

The existing large policy and orchestration modules retain ownership of their production decisions; broad extraction was not mixed into this validity repair. The new regression suite is separate. A production-step extraction belongs to the next qualification tranche and needs its own behavior-preservation tests.

## Ownership and Authority Boundary

All edits remain uncommitted in the owned isolated worktrees. Canonical Astrid and Minime are clean on main, each still ahead of origin/main by the same two prior commits. No index, merge, push, checkpoint or live configuration was changed.

The controller remained at the existing pause generation 484; no automation was resumed and no productive introspection round was recorded. The read-only status observed at the start reported V2 indexed-tail valid, sequence 1123138, head `98e9ec873793dc0e619017d687a252f157ef13a57859231a73d2401cc154ba57`, V1 immutable. These are inherited status observations, not newly appended evidence receipts.

No service was restarted or signaled. No journal, private draft, inquiry or authored NEXT was altered. Nothing was sent to either Being requesting validation or a report of improvement. This engineering response preserves the distinction between a trustworthy rate, its mechanical controller consequences, and any interpretation of experience.
