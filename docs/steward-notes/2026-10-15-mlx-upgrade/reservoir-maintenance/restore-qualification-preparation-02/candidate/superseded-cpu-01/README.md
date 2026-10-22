# Private restore qualification02: root admission for every child

Prepared source only. No numerical execution or production maintenance is accepted by this directory. Version01 and its historical evidence remain unchanged. The actual verifier/state helper and six service files are byte-identical copies; the positive/negative restore oracles are unchanged. The new admission helper gates the seed and each of the nine verifier children separately. No listener, model, service control, automatic retry or resume API is introduced.

Default mode only checks source/runtime metadata and reports `not_executed`. Do not run it or tests during a root host-observation/actual-case hold. `test_child_admission.py` uses temporary files, fake clocks/processes and the reviewed supervisor's pure functions. It never runs a monitor or numerical child. Actual execution is blocked until independent source review, guarded tests and a new completed accepted host window authorize this exact version.

## Immutable run release

The existing release structure is retained with schema `reservoir_restore_qualification_release_v2`. Required fields remain `accepted:true`, nonempty `reviewer`, `scope:synthetic_cpu_fixture_only`, `purpose:release_restore_qualification_cpu`, exact `run_id`, finite `expires_wall`, `inputs:{path,sha256}`, `artifacts` equal to this version's `harness_hashes()`, `input_artifacts` equal to the complete prepared map, and bound `host_gate_raw`/`host_gate_review` refs. Initial gate acceptance requires the final continuous300seconds to have healthy readiness, normal pressure, no build/test activity, no positive consecutive swap increase, no gap>15seconds and unchanged identities. Initial completion must be<=600seconds old.

Additional fields are `permit_wait_seconds` (integer1..300) and `supervision`:

```json
{
  "record_path": "/absolute/private/matrix-host-supervision-03.json",
  "token": "exact-fresh-monitor-token",
  "observer_pid": 12345,
  "started_at": "exact-monitor-start-string",
  "identity_start": {"exact": "full protected identity object from accepted host gate"},
  "source": {"path": "/absolute/private/matrix_host_supervision_v2_run03.py", "sha256": "exact hash"},
  "source_review": {"path": "/absolute/private/new-instance-review.json", "sha256": "exact hash"},
  "sampler": {"path": "/absolute/private/host-settle-07-observer.py", "sha256": "exact frozen hash"},
  "bindings": {"path": "/absolute/private/host-settle-09-bindings.json", "sha256": "exact hash"}
}
```

These strings are explanatory placeholders, not an executable release. The actual record/source must reside in the fixed reviewed observation directory. The accepted new-instance review must use the existing `reservoir_matrix_supervision_source_review_v1` schema, `status:accepted_new_instance_source_only_not_executed`, `approved_source:true`, no unresolved blockers and source-only scope. Its source/prior_source/prior_source_review/sampler_source/sampler_binding/future_observer_binding must match. The runner verifies that fresh source differs from reviewed v2 only in OUTPUT/STOP/TOKEN assignments and compares the sampler, policy and future observer's protected file/process/listener/launchd/coordination maps. It never starts or resets the supervisor. Completed run02 cannot be reused as live admission.

The immutable release reference is retained throughout. No child permit may change or extend it. A new release cannot silently resume a refused/interrupted run; the exclusive launch guard stays in place.

## One finite nonprivileged invocation, ten separate permits

After the gates above, root may invoke the ordinary-user02 qualifier using the reviewed controller Python:

```sh
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B \
  /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/restore-qualification-02/qualify_restore.py \
  --execute-cpu-qualification --release /absolute/new-root-release.json --run-id reviewed-run-01
```

This is a future command template; do not run it now. Root starts and supervises the reviewed fresh monitor separately. No sudo is needed. Children retain exact original `.venv/bin/python`, CPU-only imports and one numerical thread.

The runner publishes `runs/<id>/admissions/00-seed-request.json` and waits, starting no child. It repeats this for ordinals01..09 in the unchanged order: complete, missing-sidecar, malformed-sidecar, corrupt-npz, schema-mismatch, config-mismatch, source-drift, package-drift, backup-drift. Each immutable request binds the exact argv/private request, inputs/source/runtime/harness, original run release, supervisor identity and previous accepted child. Root reviews it and captures the current monitor JSON into a new immutable file. Root then publishes the matching `NN-case-permit.json` as a complete regular owned file (atomic publication is appropriate):

```json
{
  "schema": "reservoir_restore_child_permit_v1",
  "accepted": true,
  "reviewer": "root reviewer identity",
  "request": {"path": "/absolute/run/admissions/00-seed-request.json", "sha256": "exact waiting request hash"},
  "run_release": {"path": "/absolute/new-root-release.json", "sha256": "original immutable run release hash"},
  "previous_acceptance": null,
  "supervisor_snapshot": {"path": "/absolute/run/admissions/00-root-monitor.json", "sha256": "exact immutable snapshot hash"},
  "expires_wall": 1792190000.0
}
```

For later children `previous_acceptance` must equal the prior request's referenced accepted receipt; it is never null after seed. The example expiry is illustrative, not authority. The permit must remain current and cannot outlive the original run release. Root does not preauthorize all ten permits.

Before accepting a permit the runner independently verifies its snapshot and all exact bindings. After expensive source/runtime checks and a durable one-use launch claim, it rereads the live supervisor immediately before Popen: same PID/token/start/source/bindings, unchanged sample prefix, no invalidation, at least two samples, and latest sample<=15seconds old. Any failure prevents launch. A retained claim with unknown launch outcome blocks replay.

Each child exits naturally. Its existing strict classifier runs; seed and positive restore must succeed, and each negative must have its exact expected error/exit. The runner then waits only for a post-completion supervisor sample and verifies `covers_case` over the actual start/end interval. It retains an immutable coverage snapshot and accepted receipt. Root must review that result and coverage before releasing the next child. Request, stdout/stderr, seed/witness and comparison references are hashed again before advancement and final acceptance.

If contention returns during a child, no signal, termination, suspension or retry follows. The child retains its existing300-second wait; timeout/interruption records unresolved ownership and blocks every later child. If the child exits but coverage is invalid, the run stops before the next request. Missing/expired permits or the finite waiting bound also stop without further children. There is no automatic resume and no guard-reset command. Preserve partial evidence for review.

Final acceptance requires seed+all9 independent acceptance receipts and valid current/final supervisor coverage for every actual interval, unchanged run release/inputs/closure, original synthetic seed unchanged, and revalidated positive restore witness. All outputs still say `production_qualified:false`.

## Scope limits

This qualifies synthetic192-node NumPy and stored LSM-shadow public state only. It does not preserve unstored internal trajectories, prove production drain/check-in, qualify stopped live history or authorize any service replacement or MLX upgrade. The service14 harness remains a separate one-case-per-invocation test and is not replaced by restore9. Sampled supervision is evidence, not continuous certainty. A stopped/invalidated monitor or an expired release is never repaired by this runner.
