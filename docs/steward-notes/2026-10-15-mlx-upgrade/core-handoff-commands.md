These commands prepare the first production core upgrade. **They are not ready to run as an activation:** the required accepted proof at `/Users/v/other/astrid/.runtime/mlx-upgrade/intake/core-qualification.json` does not yet exist. Current host memory readiness and the remaining qualification gates must pass before that proof is produced. This document does not supply or waive those gates. No command below was executed while preparing it.

The target is `manifests-v2/core.json`. The tested comparison baseline is the isolated clone in `manifests-v2/baseline.json`; the preserved rollback environment is the original shared `.venv`, selected by `manifests-v2/original.json`. The control directory is `/Users/v/other/astrid/.runtime/model-environment`.

After an accepted core qualification proof exists, run this check as the normal user. It performs validation without creating a transition or signaling the provider; it requires no administrator operation.

```bash
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B \
  /Users/v/other/astrid/scripts/model_environment_activate.py check \
  --control-root /Users/v/other/astrid/.runtime/model-environment \
  --manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/core.json \
  --baseline-manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/original.json \
  --qualification-baseline-manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/baseline.json \
  --fixture-proof /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/launchd-handoff-fixture-admin-02.json \
  --qualification-proof /Users/v/other/astrid/.runtime/mlx-upgrade/intake/core-qualification.json \
  --expected-pid 3893 \
  --actor codex-mlx-upgrade \
  --ack "User-authorized first core MLX handoff after accepted qualification" \
  --migrate-launcher
```

Only after that check succeeds and the coordinator confirms the accepted proof and current readiness, run the following in **Terminal as the normal user**. Keep Terminal open until the controller returns. `--sudo-debug` permits an administrator password prompt for the first migration's single `launchctl debug` operation; the controller and held launcher retain the normal user's identity. **Never prefix the whole command with `sudo`.** A separate `sudo -v` in another terminal is not a dependable credential handoff.

```bash
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B \
  /Users/v/other/astrid/scripts/model_environment_activate.py activate \
  --control-root /Users/v/other/astrid/.runtime/model-environment \
  --manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/core.json \
  --baseline-manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/original.json \
  --qualification-baseline-manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/baseline.json \
  --fixture-proof /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/launchd-handoff-fixture-admin-02.json \
  --qualification-proof /Users/v/other/astrid/.runtime/mlx-upgrade/intake/core-qualification.json \
  --expected-pid 3893 \
  --actor codex-mlx-upgrade \
  --ack "User-authorized first core MLX handoff after accepted qualification" \
  --migrate-launcher \
  --sudo-debug \
  --timeout-secs 1500
```

The controller enforces the fixture, release/package/source hashes, accepted qualification, baseline process identity, paused stewardship with no active lease, idle boundary, graceful drain, held replacement, readiness, and deployment receipt checks. It does not force an overdue drain. A timeout or failure may retain an owned hold and transaction evidence; have the coordinator inspect the exact transaction before recovery. Do not delete holds, retry with guessed PIDs, or restore older runtime history. A rollback command must be prepared against the then-current process and retained transaction, using the preserved original manifest.

These command identities expire if provider PID/start changes, the host reboots, or a bound helper, interpreter, manifest, package, or evidence file changes. A changed provider requires a fresh relevant baseline; a changed boot or fixture binding requires a newly qualified owned fixture. Do not simply substitute a new PID or interpreter path into these commands.

The concrete bindings are:

- [Administrator fixture 02](/Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/launchd-handoff-fixture-admin-02.json), SHA256 `f5cc7bf3f0fa2208ba9327c4e5504bce8cd51e81bf451b32b11f2c5275b44042`: passed on host `m3-volya`, UID 501, macOS 27.0.1, boot `A86C8146-975D-4935-85A6-A7879EC7BBE3`, using the exact Python above; cleanup confirmed the owned fixture service absent. Its [review](/Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/launchd-handoff-fixture-admin-02-review.json) retains the administrator fixture interpretation.
- [Frozen controller](/Users/v/other/astrid/scripts/model_environment_activate.py:724), SHA256 `3523eadcc729004908ad99419e17b45c25245330dc0066e45f40cb558622dc86`; [launcher](/Users/v/other/astrid/scripts/model_environment.py), SHA256 `5d9ae37877e798684295b1bfa5128f99a14aecc5d4eefe863dc93e0138877587`; [owned fixture helper](/Users/v/other/astrid/scripts/qualify_model_launchd_handoff.py), SHA256 `570e7f40ebc778a488b4179eee81f6e3122569617aa9826f797fe5a5ed25ac00`.
- [Final CPU/tooling evidence 07](/Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/focused-tests-final-07.json) binds the 32-file tooling closure. [Continuation identity review 02](/Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/continuation-identity-review-02.json), SHA256 `ee3467a31f11017c7dd15018863dc779a768aaba44ce932b7013d10929845beb`, rechecked that closure, all 105 baseline files, the administrator fixture, and unchanged provider PID 3893/start `Wed Sep 30 14:29:07 2026`. Historical identity compatibility is separate from current memory readiness and accepted candidate qualification.
