# One-use abort of the failed owned fixture

This is a separately owned cleanup of the exact failed synthetic fixture
`4327212894d64b31980b98bf3fbd7fc2`. It cannot make that fixture pass, resume its
controller, qualify production maintenance, or operate on Astrid/Minime services.
The original release, owner, 141-file snapshot and failed journals are retained.
The target is fixed in source and `target.json`; there are no general target,
PID, directory or command-line overrides.

Source is prepared for independent review and root-serialized stdlib fake tests.
No execution approval is included. The default entrypoint prints a nonexecuting
status without probes, writes or imports of any previous fixture/production code.
`--check` is a read-only preflight: it holds the existing original execution flock,
verifies retained files and runs bounded read-only identity/listener queries. It
does not create an abort root. Root must separately authorize this preflight.
Do not run the controller with sudo.

## Finite effect order

1. Verify the selected Python alias still resolves to the pinned wrapper/framework
   pair, exact host/boot, all frozen source and protected inputs, pause 528/no lease,
   old parent/executor/provider absence, unique completed 11-tick check-in, exact
   provider holder and original reservoir worker/sentinel. Reserve a new abort
   root and immutable attempt/owner/ledger without rewriting original ownership.
2. Create a new abort-only reservoir guard. Invoke only the exact reservoir
   `sudo -- launchctl debug` command with inherited Terminal stdin. Require its
   durable successful natural return before one direct SIGTERM to reservoir 9217.
   Immediately before each control effect, recheck its exact worker/holder, guard
   and listener conditions after full source/ownership validation.
3. Use a 90-second acceptance deadline for cooperative save/hold observation. An
   exact old worker, loaded/no-PID gap, or exact
   pinned holder launcher with its full bound tail may still be pending. Hold
   acceptance requires the final framework argv, stable PID/start, exact guard
   and held receipt, no listeners and old-worker absence. No producer is retried.
4. Require the original log prefix/inode and exactly six new reservoir lifecycle
   rows: stop, delayed final batch, both handle saves, final complete, drained.
   Bind the stop/save to this abort's signal intent, require at least 5.9 seconds
   in the worker's synchronous save boundary, and prove durable state 11/7.
   Embed exact UTF-8 state and complete reservoir log bytes with their source
   hashes in the separately fsynced save-proof record before any held removal.
5. Remove only the verified quiescent provider holder and reservoir holder, then
   require job/PID/private-port absence before deleting each exact hold guard.
   Preserve state and all original histories. Final receipt is cleanup-only.

The save/hold and post-bootout absence acceptance deadlines are 90 and 15 seconds,
respectively. They are checked before and after each observation. A final bounded
multi-query observation may finish beyond that deadline, but its result is then
rejected. These are not hard wall-clock interruption deadlines; each query retains
its separate 5-second wait and no timed-out child is killed.

Original-root changes are limited to the existing reservoir log append, new
`state.json`, normal reservoir ready-marker removal, and final original provider
guard deletion. The abort's reservoir guard, held receipt and all new evidence
live outside the original run. Arbitrary original-root writes are refused.

## Failure and command ownership

This driver is one-use. Any existing abort root is a hard stop; it has no resume,
retry, takeover or automatic cleanup path. Every state-changing action has a
durable intent before its effect. Interruption or ambiguous outcome preserves
that intent and retains jobs, guards, state and children for separate review.

Both read-only query children and action-capable sudo/launchctl children use
`Popen` without a context manager. Query `communicate` is bounded to 5 seconds;
debug wait to 180 seconds and held-only bootout wait to 20 seconds. Timeout or
interruption never sends kill/terminate and never relaunches a command. Evidence
distinguishes a known naturally reaped command from an unknown command effect.
If a query is unresolved, the driver also stops without killing or polling it.
The parent retains child objects; it does not claim to reap launchd workers.

The successful result cannot prove its own executor has exited. A later separate
read-only closeout must establish that executor and all recorded command children
are absent, with exact retained identity and clean query return evidence. Any
unresolved child or failed result requires manual review, not an automatic rerun.

## Review/release contract

Root will author the only accepted `evidence/abort-release-01.json` after tests
and independent review. Its `owned_fixture02_abort_release_v1` object must contain:
`approved: true`, `execution_authorized: true`, scope
`exact_failed_fixture_abort_only`, `production_qualified: false`, a future zoned
`not_after`, exact `artifacts` (all direct Python files, this README and target),
and path/SHA256 references `target`, `original_release`, `snapshot`, `design`,
`cpu_receipt`, and `independent_review`. The CPU receipt must have `passed: true`
and the same artifacts map. The independent review must have `approved: true`,
the same scope/artifacts and the exact CPU receipt reference. These are evidence
requirements, not self-issued authority. No final administrator command is
published at this preparation stage.

Only `abort_driver`, `abort_native`, and `abort_evidence` are imported by the
driver. Tests are `test_abort_driver`, `test_abort_native`, and
`test_abort_evidence`; their service/command boundaries are mocked and their file
fixtures are private temporary files. Root must run them under the separate
audit/import/source-preservation guard. No target module import, actual ps,
launchctl, signal, hardware, model, network or numeric execution is permitted
in that test pass.
