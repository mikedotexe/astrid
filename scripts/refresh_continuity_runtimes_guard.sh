#!/usr/bin/env bash
set -euo pipefail

bridge_pattern="consciousness-bridge-server"
minime_pattern="/Users/v/other/minime/autonomous_agent.py --interval 60"

count_matches() {
  local pattern="$1"
  pgrep -fl "$pattern" | wc -l | tr -d ' '
}

report_matches() {
  local label="$1"
  local pattern="$2"
  local count
  count="$(count_matches "$pattern")"
  printf '%s process count: %s\n' "$label" "$count"
  pgrep -fl "$pattern" || true
  if [ "$count" -gt 1 ]; then
    printf 'Refusing refresh: duplicate %s processes detected.\n' "$label" >&2
    return 1
  fi
}

report_matches "Astrid bridge" "$bridge_pattern"
report_matches "Minime autonomous agent" "$minime_pattern"
printf 'Continuity runtime guard passed. Restart commands remain manual and explicit.\n'
