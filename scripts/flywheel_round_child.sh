#!/bin/bash
# flywheel_round_child.sh — the child process run by steward_control.py's
# subprocess adapter for one bounded introspection-flywheel round. Invoked by
# scripts/flywheel_loop_run.sh; do not run directly (it assumes the adapter
# already holds the controller lease).
set -u

export PATH="/Users/v/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
PROMPT_FILE="${FLYWHEEL_LOOP_PROMPT_FILE:-$ASTRID/scripts/flywheel_loop_prompt.txt}"
MODEL="${FLYWHEEL_LOOP_MODEL:-opus}"

cd "$ASTRID" || exit 1
[ -f "$PROMPT_FILE" ] || { echo "flywheel child: prompt file missing: $PROMPT_FILE" >&2; exit 1; }

exec claude -p --dangerously-skip-permissions --add-dir "$MINIME" --model "$MODEL" < "$PROMPT_FILE"
