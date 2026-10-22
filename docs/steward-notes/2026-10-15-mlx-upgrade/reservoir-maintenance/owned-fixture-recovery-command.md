# Finish owned fixture recovery and test the corrected adapter

Run this once in Terminal as the logged-in user. Do not prefix it with `sudo`:

```bash
/bin/zsh -f /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/candidate-v2/continue_owned_fixture.zsh
```

The script first finishes cleanup of the exact retained private fixture. If that
succeeds, it runs a fresh complete fixture using the reviewed inspection repair.
It stops on any error and preserves results; do not run it again blindly.
It should take about a minute after authentication. Leave Terminal open until it
returns to the prompt, and enter any administrator password only there.

The earlier user-run fixture completed its main transaction, then failed during
cleanup when the provider exited between two process queries. Its result remains
failed and is archived under `launchd-admin-01-failure/`. The v2 repair and exact
recovery passed 88 isolated tests and independent source review.

One noninteractive recovery attempt removed only the positively verified
provider holder PID 77047. The provider job is absent. The reservoir worker PID
76424, started `Fri Oct 16 08:02:53 2026`, remains running with the second
acknowledged synthetic check-in. Its owned guard is retained. `sudo -n` refused
with `sudo: a password is required` before executing `launchctl`; the worker was
not signalled and no next-spawn override was installed by that attempt.

The continuation authenticates only the narrow `launchctl debug` operation for
the owned fixture label, then allows the existing reservoir worker to complete
its final save. It verifies final synthetic Astrid ticks 22 and Minime ticks 7,
the unchanged original backup at Astrid ticks 11, and absence of both owned jobs.
It does not elevate Python or target a production label.

Expected new results:

- `.runtime/mlx-upgrade/reservoir-maintenance/evidence/fixture-recovery-admin-01.json`
- `.runtime/mlx-upgrade/reservoir-maintenance/evidence/launchd-fixture-admin-02.json`

Both must actually report success and pass provenance/cleanup review before the
fixture is accepted. No outcome here qualifies real reservoir persistence or
promotes MLX. The production adapter, real restore verification and numeric
matrix still remain, followed by a new clean two-hour MLX baseline.

Bindings and evidence are in the adjacent `implementation-02/` archive. All six
protected production process identities and live reservoir/persistence source
hashes matched after the noninteractive attempt. The stewardship pause and other
paused automations remain in place; this follow-up is paused for authentication.
