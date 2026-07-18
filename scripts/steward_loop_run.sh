#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  '{"schema":"steward_control_legacy_entrypoint_v1","entrypoint":"steward_loop_run.sh","retired":true,"mutated":false,"replacement":"scripts/steward_control.py"}' \
  >&2
exit 2
