# Owned fixture cleanup failure, October 16

The user ran the reviewed private launchd fixture. Its main transaction reached
`verified`: provider check-in completed, the reservoir finished its long final
save, both owned replacements held, and the original private settings restarted
with the preserved synthetic state.

Success cleanup then refused with `process changed during inspection`. The
provider worker completed its second unique check-in and exited between process
inspection calls. The exact provider holder (PID 77047) and reservoir worker
(PID 76424) were retained. This is a failed fixture result, not host acceptance.

The reservoir's event record contains Astrid ticks 22 and Minime ticks 7 after
the second check-in. Its saved state and original preserved backup still contain
Astrid ticks 11 until the final reservoir drain. Recovery must finish this
existing cleanup; it must not replay the check-in or restart the provider.

The manifest preserves 93 files before recovery, including the failed result,
all 27 host operations, completed transaction journal, both check-in records,
held receipts, and synthetic state. This is an incident snapshot of a running
fixture, not a stopped-service backup. Original helper bytes remain unchanged;
the inspection repair and exact cleanup recovery are prepared in a separate
versioned candidate.

All six protected production PID/start/command identities matched the previous
baseline reference at incident review. No production signal, package update,
source installation, reservoir-state mutation, or MLX promotion occurred.
