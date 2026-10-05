# Maintenance-Only Minime NEXT Transition

## Scope and Authority

Mike approved continuing the legacy transition and deployment/integration on
2026-10-05. He also explicitly approved a bounded model-qualification window:
hold the two text callers, gracefully stop only the coupled provider under a
reversible pre-model launch hold, run isolated tests if resource bounds permit,
and restore prior services. The approved pilot uses a 10 GiB process stop
threshold and 4 GiB free-memory reserve; the candidate admission rule requires
16 GiB startup headroom. Polling is not a hard allocation cap.

This first release is narrower: Minime's Python agent and operator tooling only.
The bridge, shared reader/helper, text providers, engine, models, visual service,
and sensory clients are not restarted. Writing limits and research policy remain
at the existing main baseline. Previously paused automations stay paused.

The larger capacity candidate cannot be installed just to obtain a maintenance
protocol: its clients require providers that have not completed real-model
qualification. The maintenance port has only four changed runtime source inputs:

- `minime_autonomy/runtime.py`
- `minime_autonomy/collaboration_attention.py`
- `minime_autonomy/maintenance_next.py`
- `minime_autonomy/maintenance_next_legacy.py`

## Preservation Contract

The existing sovereignty file remains the authoritative queue. New runtime
admissions establish a durable hash-only unsealed-admission marker before
clearing a choice. Shared writers serialize under the same owner lock; stale
writers cannot erase a newer pending tuple. Shutdown joins accepted workers,
retains unadmitted work and seals exact queue bytes. Startup acknowledges the
exact command, selection identity, envelope and original timestamp before
profile-specific restoration or a competing boot reflection.

The legacy process cannot retroactively author a new-protocol seal. The new
`minime_legacy_transition.py` records a separate operator-observed import after
the existing sanctioned reload wrapper observes completed drain and exit. It
freezes the stopped queue hash, old process/source/status identity and expected
replacement source identity. The new runtime writes a distinct `legacy_restored`
acknowledgement. Both records explicitly deny a runtime seal and execution
confirmation. No command payload or private prose appears in these receipts.

The tool freezes reviewed sources and source backups before stopping. It may
install only the four maintenance paths after exit, with per-file intent records.
It never rewrites the queue or restores historical backups over current work.
Interrupted installation can resume only from exact old/new file hashes and an
unchanged stopped queue. Ambiguous release, corrupt history, foreign activity,
active jobs or changed identities fail closed. No forced termination or
automatic rollback is implemented.

After a new-protocol admission, abrupt exit remains an explicit recovery-review
boundary. Do not delete an unsealed marker to force startup. Partial compound
execution is not reconstructed or automatically replayed. A preserved choice is
not evidence that it executed or succeeded.

## Qualification

- The isolated Minime port is based on main `9072abff9ed839851709300b3c3eaae3afaf669e`.
- The operator port is based on Astrid main `a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32`.
- 181 operator/restart/staging/compatibility tests pass in the isolated operator
  worktree. Tests include wrong process/source, failed process inventory, an
  interrupted install, changed stopped queue, downgrade refusal and missing
  restoration acknowledgement.
- Minime's final full run: 1,869 passed, 143 subtests passed, one existing skip.
  The preceding full run had 1,868 passes, 143 subtests, one skip and one failure.
  The failure reproduced against exact canonical source and the deployed helper.
  It was a stale assertion excluding a previous public reflection, which deployed
  commit `0ac99af41b` deliberately offers as optional context. The assertion now
  checks that existing public contract while still excluding the new private
  passage and its NEXT command. The failed attempt is retained in the Minime
  qualification note. The corrected full rerun passed in 66.52 seconds.
- Controller, projector, Evidence Event Store, Division tracker/Chronicle,
  flywheel, environment-receipt and domain-boundary support suites: 106 passed.
- Domain-boundary verification passes with zero violations; experiential
  epistemics self-tests pass (2). No Rust production change is included, so no
  new Rust build or model-capacity qualification is claimed.
- The retained live bridge stage verifies without mutation: manifest
  `9ea2d28ccaf84f4a0df3857bff7f8af4938be1d94a5e2ed8816fd63f66e2dc69`,
  binary `50b2b57a4d646373aaadc28387fe3c93a21c78951e25a31c726c1a02ad2f16c5`.

Earlier operator-port qualification failed because a fixture assumed sibling
worktree paths and two compatibility checks had not yet been carried into the
operator-only port. The test fixture now accepts an explicit Minime source root,
and the reviewed native-report compatibility checks are present. No test was
excluded to produce the 181-test pass.

## Remaining Capacity Work

This transition does not qualify 96,000-byte private inputs or 16,384-token
expressive outputs. Native/MLX real-model measurements, provider release identity,
and coordinated capacity activation remain separate. A current read-only sample
reported 5,425,168,384 free bytes and 9,266,437,440 bytes of coupled-provider
physical footprint. Their sum is not a reclamation guarantee and does not prove
16 GiB startup headroom. Do not load another model merely because the maintenance
window is approved.

## Activation and Git Record

Controller stabilization claimed at pause generation 494, with no active lease;
the earlier pause remains in force. Activation is pending. Append the exact transaction, PID transition,
source identity, queue-import/acknowledgement hashes, protected-process checks,
commit identities and remaining limitations after the actual operation.
