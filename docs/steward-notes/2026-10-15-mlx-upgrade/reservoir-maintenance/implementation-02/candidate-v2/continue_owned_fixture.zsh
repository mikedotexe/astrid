#!/bin/zsh
# Ordinary-user continuation. Only the reviewed Python helpers invoke sudo,
# narrowly for launchctl debug on generated private fixture labels.
set -euo pipefail

/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B \
  /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/candidate-v2/recover_owned_fixture.py \
  --reviewed-bindings /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/fixture-recovery-independent-review-01.json \
  --output /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/fixture-recovery-admin-01.json \
  --interactive-debug

exec /opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B \
  /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/candidate-v2/qualify_reservoir_maintenance.py \
  --run-owned-host-fixture --sudo-debug \
  --reviewed-bindings /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/fixture-v2-independent-review-01.json \
  --output /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance/evidence/launchd-fixture-admin-02.json
