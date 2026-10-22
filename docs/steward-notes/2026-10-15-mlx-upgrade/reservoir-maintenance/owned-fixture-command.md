# Owned launchd fixture: interactive command

Ready for one user-run fixture; not a production handoff. Run in a local Terminal
as user `v`. The Python controller refuses root execution. It invokes `sudo`
only for `launchctl debug` against newly generated private fixture labels, with
a durable owner hold already created. macOS requires this interactive
administrator authentication; Codex cannot supply it.

```bash
/opt/homebrew/opt/python@3.14/bin/python3.14 -I -B \
  /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/candidate/qualify_reservoir_maintenance.py \
  --run-owned-host-fixture --sudo-debug \
  --reviewed-bindings /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/fixture-adapter-independent-review-01.json \
  --output /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/launchd-fixture-admin-01.json
```

Enter the administrator password only in Terminal's normal sudo prompt. Do not
put `sudo` before Python or run a separate launchctl override. Run this command
once; if it fails or is interrupted, leave its files, jobs and holds intact and
report the result here. Do not rerun into a new output or manually remove jobs.
The controller retains the exact private ownership and operation receipts for
reviewed recovery and does not force-kill workers on error.

The expected successful run starts two small stdlib workers, drains the provider
while its private reservoir remains available, waits for each worker's natural
exit and owned held replacement, verifies saved fake state, restores the original
fixture settings, then repeats graceful draining for cleanup. No production
service, model, installed package or production reservoir state is changed.

The exact seven-artifact closure passed 63 focused fake tests and independent
review. A separate real-subprocess test passed two six-second drain cycles with
four natural exits and preserved fake state. The source review and read-only
command gate do not prove launchd behavior; the result must be inspected before
acceptance. This fixture also does not qualify the actual NumPy service matrix,
production state restore, production handoff or MLX upgrade.

Evidence: [source/test archive](implementation-01/archive-manifest.json),
[independent review](implementation-01/fixture-adapter-independent-review-01.json),
and [owned subprocess result](worker-integration-01/archive-manifest.json).
