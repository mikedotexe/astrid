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
the earlier pause remains in force. Both canonical trees and remote main tips
were checked twice, with no foreign cooperative activity. Explicit-path commits:

- Astrid `1e70116b944e257546213fbe78abfb4cd2eeddc5`, operator implementation,
  tests, changelog, ledger and this note; fast-forwarded to local main.
- Minime `affae09827ef57628b143ddf3b4ac6dec8f13267`, four runtime paths,
  focused tests, existing fixture corrections, changelog and qualification notes;
  fast-forwarded to local main after verifying the installed runtime bytes.
- Staged-state Minime rerun, now using the maintenance-only Astrid proof helper:
  1,869 passed, one existing skip, 143 subtests in 68.18 seconds.
- Staged-state operator rerun: 181 passed in 4.894 seconds.

Actual sanctioned transaction:
`/Users/v/other/worktrees/writing-capacity-next-budget-20261002/maintenance-next-live-20261005-01`.
Its frozen `manifest.json` SHA-256 is
`e5743803b381ffa3a602ea295029de62b9f12f8b14749cebcad8c4714785fef9`.
The private `completed.json` SHA-256 is
`3616be9e54276b72cc3d6bd77d86464eb805cd4f69c19460058a01decc23cc47`.
The source snapshot, original source backups and append-only events are retained
there. They are not an instruction to overwrite later state.

The wrapper waited for idle, established its launch hold, sent one SIGTERM to
79523 and observed `exited` / `drained`. No force was used. Replacement 76377
started at `Mon Oct 5 13:35:59 2026` according to the process table; runtime
initialization recorded `2026-10-05T13:36:27`. These are different clocks, not
interchangeable timestamps. Readiness and all 92 loaded source inputs matched:
`d29138949e5f1bd07f53bf88c65d696ff0912f80b7694d0b5eaf2621068cc378`.
`reload_required=false`; no new interrupted jobs were found.

The stopped queue was **empty**. The exact stopped-state import and replacement
acknowledgement prove that observation, not preservation of a nonempty live
choice in this particular transition. Synthetic tests cover nonempty choices.

- Import SHA-256: `8a594c214b2b82ccd7a4ed1754137e18d3c76e2c014b3566a3ca4967a667515f`.
- Runtime acknowledgement SHA-256: `de0231e597bb191fa399154946f315a8a3771fc513de354bb29469f2f7a2cdb6`.
- Boot ID: `4408c254952e4c08b8ed8a0e9711e034`.
- `runtime_seal_available=false`, `execution_confirmed=false`; no held queue
  text was exported, changed or automatically manufactured.

Managed configuration and every protected launchd PID/start identity matched the
preparation. Bridge 4732, coupled provider 3893, reservoir engine 35303 and shared
Ollama 9458 retained their starts. The bridge stage and reader helper were
reverified unchanged; helper SHA-256:
`af632204fd96760a413f73bbd00fbc54f6638bf163eb36bb23abde995dc296fe`.
Visual, sensory, gateway and supervisor identities also matched the transaction.
The owned launch hold is absent after successful release. Sampled fill during
this transition was 71.04-73.03%, with fresh telemetry; this bounded observation
does not establish long-term controller stability or felt improvement.

An initial prepare invocation refused before creating a transaction or changing
live state because the isolated worktree's default controller path did not exist.
The successful invocation explicitly selected the canonical controller via
`ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid`, validating pause generation 494.
No missing controller file was created or substituted.

Post-transition controller verification: no lease or projection, generation 494
still paused; V2 valid, sequence 1,123,148, head
`0835ca66c1048d02723201afb7133bb8082ad3835c58f28c2e81205189e062f0`;
legacy V1 sources immutable. This was not a productive introspection round.

## Remaining Provider Boundary

Maintenance-window approval is granted, not awaiting another generic approval.
The separate production caller-stop implementation is still hard-disabled while
control-journal retry/headroom integration, shared callback deadlines and writes
outside its reservations remain unqualified. The new running NEXT capability
removes one prerequisite; it does not remove those others.

Three later native-counter samples reported free bytes of 4,153,540,608,
3,969,204,224 and 3,399,565,312, below the approved 4 GiB reserve. Provider 3893's
physical footprint was 9,306,053,952 bytes. There is no verified 16 GiB startup
headroom or reclamation guarantee. No isolated model was loaded, no provider was
stopped, and no four-cell real-model result is claimed. Keep the larger candidate
isolated until those concrete gates pass; do not activate capacity clients now.
