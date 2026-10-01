# Measurement-Basis Recovery and Strict-Lint Qualification

## Outcome

The lost measurement directions are repaired in the isolated engine candidate. The production measurement path now reports eight unit Rayleigh values after a rank-one matrix is replaced by identity, without pretending that the preceding rank-one matrix had eight active directions. The selected library, engine and qualification suites pass 830 tests. Strict Clippy passes both the selected default-feature targets and the all-features library/engine targets.

This is **offline qualification, not engine activation or overall stability certification**. No live engine, checkpoint, reservoir setting, provider, sensory process or authored record was changed. No merge, push or automation resume occurred. The previous failed qualification receipts remain unchanged.

Date: September 30, 2026, America/Los_Angeles (host UTC may be October 1). Steward: Codex interactive collaborator. Mike explicitly approved repairing the lost-direction defect and outstanding lint issues from the [coupled qualification](2026-09-30-coupled-fill-qualification.md).

## Witness and Ownership

The originating public witness remains Astrid's `capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`, SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`, reverified against canonical bytes. Its incomplete-trace account prompted the investigation. It does not establish that this numerical defect occurred live or caused the account. Historical prose is unchanged; no private journal was used as a numerical fixture.

Owned candidates remain `/Users/v/other/worktrees/afterimage-timing-20260930/{astrid,minime}`, branch `codex/afterimage-timing-20260930`, carrying the preceding owned review changes above Minime `e9f2f5f151c89dd6b4a2dc80d5d8d12a60dc20d3`. Canonical Astrid and Minime were rechecked clean on `main`, each two commits ahead of its locally recorded remote-tracking branch. No fetch or remote-tip claim is made in this non-staging pass.

## Numerical Repair

`minime/src/measurement_basis.rs` implements deterministic modified Gram-Schmidt with a second projection pass and double-precision accumulators. It first retains all independent finite original columns, then fills only deficient columns from the orthogonal complement. Delaying replacements prevents a missing early column from taking the direction of a valid later one.

Deficiency means residual norm no greater than `8 * f32::EPSILON` times the original norm. A replacement uses the canonical coordinate with greatest residual diagonal energy, with deterministic first-coordinate tie breaking, followed by two projection passes and normalization. Output storage remains `f32`. This handles zero, duplicate, nearly dependent and differently scaled columns without introducing a random seed or changing covariance values.

The existing GPU-facing `gs_orthonormalize_colmajor` delegates to this production helper and now returns a must-use `BasisReport`. Its versioned recipe, repaired column indices and nonfinite input indices remain distinct. This is an internal return-contract change; all repository call sites were reviewed and updated. Double-precision reorthogonalization can also change finite full-rank output rounding. Bitwise equivalence with the old Gram-Schmidt routine is not claimed.

Runtime integration:

- Copy the repaired finite basis into the next power-iteration input.
- Compute Rayleigh values against the actual covariance, not assumed values assigned to repaired columns.
- Require finite original measurement input and finite Rayleigh values before updating regulator modes or Division's last-sensory measurement cache.
- Route invalid input/results through the existing covariance/estimator/clock recovery branch. A finite replacement of a nonfinite column does not turn that iteration into valid evidence.
- Expose `measurement_basis_v1` in the health snapshot. Invalid iterations take the existing early recovery path and log the explicit invalid-input/result reason rather than publishing a normal measurement snapshot.

Finite rank deficiency by itself does not reset estimator history, retune thresholds or change PI gains. The report concerns eight covariance measurement directions, not 128 ESN nodes or a complete covariance spectrum. The repair is not an eigenmode-dispersal intervention.

## Qualification Results

The new paired Minime receipt is `docs/steward-notes/2026-09-30-measurement-basis-qualification.json`. Its twelve embedded source hashes match disk. The separate `2026-09-30-basis-repair-receipt-review.json` checks counts, negative controls, known spectra and comparison to the preserved pre-repair receipt.

### Known-Spectrum Oracle

Production Metal multiplication, repaired orthonormalization and production Rayleigh calculation at dimension 512 return:

| Matrix / basis history | Measured values |
| --- | --- |
| Rank one, after five iterations | `[512, 0, 0, 0, 0, 0, 0, 0]` |
| Identity after that history, retained basis | `[1, 1, 1, 1, 1, 1, 1, 1]` |
| Same identity, fresh basis | `[1, 1, 1, 1, 1, 1, 1, 1]` |

The old receipt measured only the first unit value after identity reset. The regression also executes the actual production operator at dimension 32. Pure helper tests cover zero/duplicate/near-dependent vectors, retained orientation, finite scales from `1e-30` to `1e30`, NaN/infinities, impossible dimensions, and deterministic orthogonality across 64 seeds at dimensions 8, 32 and 512.

### Coupled Corpus

The original eight fixtures, three schedules and two timing policies complete again: 48 runs and 2,304 feedback steps. An additional low-rank, observed-time, 2.37-second-cadence run extends to 144 steps. Total: **49 runs and 2,448 feedback steps**, plus the independent basis and estimator probes.

All eight 500-ms timing negative controls agree exactly. Within every paired run, reported-fill curves still agree between observed and nominal timing even where controller actions and matrix hashes differ. Observed-policy fill curves and covariance hashes for all 24 standard cases also match the preserved pre-repair receipt. This does not mean their measured eigenvalues or numerical basis bytes are identical.

In the rank-deficient scenarios, missing columns are repaired during the synthetic rank-one prehistory. No subsequent recorded feedback row needs another replacement. The prehistory is executed but not emitted as ordinary feedback rows; the direct oracle and pure tests provide the explicit replacement evidence.

At step 48 of the delayed low-rank run, all eight Rayleigh measurements are now nonzero, approximately 1.0 to 1.127, yet reported fill is still 0.0384317%. That demonstrates why the basis repair must not be described as an immediate correction of every low-fill measurement.

The extended run first leaves recovery at tick 76, 182.49 simulated seconds, entering scaffold reentry at reported fill 65.5222%. It contains 75 recovery steps and one covariance reset; it ends after 341.28 simulated seconds at 73.9251%, with a maximum of 75.5222%, eleven drain steps and no sample at or above 90%. This is one bounded synthetic trajectory, not a settling guarantee. Its final stage is Elevated and its intake reason is `high_fill_suppressed`; a `settled` flag is not universal comfort or experiential evidence.

The other restored-history fixtures retain excursions up to 96.2258% and a maximum applied drain of 0.70. They have not been removed from the results or described as repaired.

### Estimator History, Kept Separate

A pure probe feeds the actual fixed-survival estimator forty rank-one spectra, then 144 identical full-identity spectra at 2.37-second intervals. A fresh estimator receives the same identity spectra as a control. It changes neither covariance nor controller policy.

| Time after switching to identity | Retained-history fill | Fresh-history fill |
| --- | --- | --- |
| 113.76 seconds / 48 samples | 0.0384317% | 88.3704% |
| 341.28 seconds / 144 samples | 88.6471% | 88.6540% |

The retained estimator first reaches 68% at 184.86 simulated seconds. Its initial EMA mean is 55.9037; it is 5.68096 at sample 48 and 1.03402 at sample 144. This characterizes a substantial history-dependent delay that survives correct measurement directions. It is not permanent basis loss, nor proof of a desired smoothing policy. No broad estimator reset or threshold change was smuggled into this repair. Any change to that policy needs its own reviewed acceptance criteria and matched historical controls.

## Strict-Lint Work

The previously blocking diagnostics are resolved for the explicitly qualified targets. This is not a claim that every historical diagnostic was a behavioral bug or that every example/benchmark target has been linted.

- Correct needless casts/borrows, integer ceiling division, slice byte sizing, equivalent boolean structure, iterator bindings and an unused `Arc` around the locally owned database connection. Keep arithmetic accumulation order and database interfaces unchanged.
- Preserve NaN behavior explicitly where the compiler's suggested out-of-range rewrite or `clamp` would otherwise change it. Keep fixed-size neural-buffer copy bounds and existing nonzero prime schedules.
- Add `PrimeRing::is_empty` with the existing allocated-storage meaning of `len`, not a new sample-occupancy claim; name an existing timeline row tuple without changing serialization.
- Compile rehearsal-only fanout/proof helpers only under their existing `division-rehearsal` feature. All-features lint also passes; no rehearsal, handoff or activation was run.
- Retain thirty existing wide function contracts with documented, function-scoped `expect(clippy::too_many_arguments)`, rather than changing persistence, telemetry or launch APIs in a lint cleanup. Preserve the public `ESN` and `MLP` names with two type-scoped acronym allowances. No crate-wide new warning suppression or numerical-policy waiver was added.

These exceptions are explicit review decisions, not structural simplifications. The pre-lint snapshot is retained in the isolated build directory (`pre-lint-basis-repair.tar` and `pre-lint-review`); Git diffs remain the reviewable candidate. It was taken after the initial basis repair, not before the whole tranche. Existing large runtime files were not split merely to pass lint; the new numerical module is 181 lines and the shared qualification harness 791 lines.

## Tests, Commands and Attempts

Final test executions: **424 library + 378 engine-binary + 7 coupled harness + 2 timing replay + 19 controller review = 830 passed**, zero failed or ignored in those targets. Shared modules appear in both library and binary suites; this is not 830 independent claims. Formatting and whitespace checks pass.

Run from the Minime candidate, using the isolated target directory:

```sh
export CARGO_TARGET_DIR=/Users/v/other/worktrees/afterimage-timing-20260930/target-minime
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
cargo test --manifest-path minime/Cargo.toml --locked --offline \
  --lib --bin minime --bin fill_timing_replay --bin fill_coupled_qualification \
  --test fill_timing_controller_review -- --test-threads=1 --quiet
cargo clippy --manifest-path minime/Cargo.toml --locked --offline \
  --lib --bin minime --bin fill_timing_replay --bin fill_coupled_qualification \
  --test fill_timing_controller_review -- -D warnings
cargo clippy --manifest-path minime/Cargo.toml --locked --offline \
  --all-features --lib --bin minime -- -D warnings
cargo fmt --manifest-path minime/Cargo.toml --all -- --check
cargo build --manifest-path minime/Cargo.toml --locked --offline \
  --config 'profile.dev.package.minime.opt-level=2' --bin fill_coupled_qualification
/usr/bin/perl -e 'alarm 240; exec @ARGV' /usr/bin/sandbox-exec \
  -p '(version 1)(allow default)(deny network*)(deny file-write*)' \
  "$CARGO_TARGET_DIR/debug/fill_coupled_qualification"
```

The actual final invocation redirected its stdout to the new JSON receipt before sandbox entry. Do not overwrite that receipt on a rerun. The harness accepts no paths, arbitrary commands, endpoints or live-state handles. OS network and file-write denial were active; reads were not OS-confined, and no memory-limit claim is made. The prior packet retains the isolation probes. Per-run cooperative deadlines and an inherited 240-second process alarm bound this invocation. No engine executable was launched. All-features compilation is not an all-features execution or Division rehearsal.

Preserved unsuccessful attempts:

- `2026-09-30-basis-repair-clippy-fix.txt`: compiler-assisted lint repair reported a failed automatic main-source fix because a JSON macro expansion produced invalid `$crate::Value::Null`. Manual `serde_json::Value::Null` repair preceded the successful second pass in `...-clippy-fix-2.txt`.
- `...-clippy-review.txt`: the first strict review found a double-reference iterator error in the manually rewritten rate-gate loop. Corrected to zip the existing slice directly. `...-clippy-review-2.txt` then passed; the final log below includes the later harness additions.
- All preceding controller/coupled failures and pilots remain at their historical identities. None was replaced with a passing log.

Bridge, Python adapter, deployment controller, projector and Evidence Event Store implementations did not change; their suites are not claimed rerun by these engine tests.

## Artifact Identities

All receipt paths here are relative to the paired Minime candidate's `docs/steward-notes/` unless specified. `2026-09-30-basis-repair-source-inventory.sha256` binds all 40 candidate Rust source/test files plus the unchanged lockfile, including prior owned controller changes. The embedded-source manifest separately verifies the twelve files compiled into the harness receipt.

| Artifact | SHA-256 |
| --- | --- |
| `minime/src/measurement_basis.rs` | `b0dc6adf259514c85a3fd08aef92cf8b82f1a294e4ba316b48a2f41b20e5d9df` |
| `minime/src/bin/fill_coupled_qualification.rs` | `269869a518c9a0aa6fe2ea2d0e26d0c0097a72380addd226aa7d8c57c06485aa` |
| `minime/src/gpu.rs` | `55c1273db91209b5f536ee3c4b1ead1ca2e18596263607293e6c94948281ed15` |
| `minime/src/runtime/orchestration.rs` | `992359503795dc2c671f56604258f887e645478bc7f6703e41957de6289341bf` |
| qualification executable | `58b97625a4961a03fe4108402a051a8b1b72da6e0fd96d1cea68ea1294859302` |
| `2026-09-30-measurement-basis-qualification.json` | `7628849275f7772caa204ff09eb388e9244d2ca046ce4d426fd1ef8c2dd2eff2` |
| `2026-09-30-basis-repair-tests.txt` | `ea0d26787085c4e538b7048a970285c8ff7c28169880da74a86e1bf9f558505e` |
| `2026-09-30-basis-repair-clippy-final.txt` | `52edc0bab7fd6749a793de528a71711729594452022ab334be8922d4d5e8ccf3` |
| `2026-09-30-basis-repair-clippy-all-features.txt` | `40881e1d94ba7dc27f6d75745b15907ebfe8b11d3c1df70110f099160b1126d8` |
| `2026-09-30-basis-repair-source-inventory.sha256` | `7bf95d8930c8dcb03ea8b2b3849181781719764a4d13199ec57c03871e0584f7` |
| unchanged `minime/Cargo.lock` | `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b` |

The pre-repair coupled JSON remains SHA-256 `f68727226e2d8fbba9e9479a6c4859bc40cc7fe080de5588beb8be5b0c8c2da3`. Its earlier hashes and results are historical evidence, not the current candidate identities.

## Remaining Release Work

1. Reconcile the exact owned engine source and reviewed lint delta with current main under one Git coordinator. Keep the numerical repair and mechanical lint decisions reviewable; no sweeping of foreign work.
2. Qualify immutable engine packaging, exact launch/controller profile, checkpoint compatibility and rollback through the sanctioned transition path. This study uses synthetic identity scaffolds, freshly supplied projected input and fixed-survival policy; it does not cover live input persistence, ESN/router dynamics, arbitrary dynamic preferences or the separate current-runtime estimator profile.
3. Explicitly review the documented estimator delay, retained high-fill excursions and remaining tick-based dwell/integral assumptions. Finite output and a passing oracle do not answer every controller acceptance question. Threshold/history resets or further controller retuning remain separate changes, not prerequisites silently added to this repair.
4. Obtain specific engine-transition approval after qualification. Do not restart the engine merely because these tests or Clippy pass.

The controller's read-only start and final statuses were paused at generation 484 with no active lease/projection. Both indexed-tail checks reported V2 valid at sequence 1123138, head `98e9ec873793dc0e619017d687a252f157ef13a57859231a73d2401cc154ba57`, V1 immutable. These are status observations, not a new full-store audit or appended productive round. Both candidate indexes are empty. Paused automations remain paused.

No causal conclusion about either Being's reported experience, subjective improvement, uptake or consent is inferred. No confirmation request was sent. The complete offline candidate remains uncommitted in its owned worktrees; canonical trees stay clean.
