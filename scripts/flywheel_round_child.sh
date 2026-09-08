#!/bin/bash
# flywheel_round_child.sh — the child process run by steward_control.py's
# subprocess adapter for one bounded introspection-flywheel round. Invoked by
# scripts/flywheel_loop_run.sh; do not run directly (it assumes the adapter
# already holds the controller lease).
set -u

export PATH="/Users/v/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

ASTRID="${FLYWHEEL_ASTRID_ROOT:-/Users/v/other/astrid}"
MINIME="${FLYWHEEL_MINIME_ROOT:-/Users/v/other/minime}"
PROMPT_FILE="${FLYWHEEL_LOOP_PROMPT_FILE:-$ASTRID/scripts/flywheel_loop_prompt.txt}"
MODEL="${FLYWHEEL_LOOP_MODEL:-opus}"
CLAUDE_BIN="${FLYWHEEL_CLAUDE_BIN:-claude}"
COMPLETION_HELPER="${FLYWHEEL_ROUND_COMPLETION_HELPER:-$ASTRID/scripts/flywheel_round_completion.py}"

cd "$ASTRID" || exit 1
[ -f "$PROMPT_FILE" ] || { echo "flywheel child: prompt file missing: $PROMPT_FILE" >&2; exit 1; }
[ -f "$COMPLETION_HELPER" ] || { echo "flywheel child: completion helper missing: $COMPLETION_HELPER" >&2; exit 1; }
[ -n "${STEWARD_RUN_ID:-}" ] || { echo "flywheel child: controller run identity missing" >&2; exit 1; }
[ -n "${STEWARD_ACTOR:-}" ] || { echo "flywheel child: controller actor missing" >&2; exit 1; }

MARKER_DIR="$(mktemp -d /tmp/astrid-flywheel-round.XXXXXX)" || exit 1
chmod 700 "$MARKER_DIR" || exit 1
export FLYWHEEL_ROUND_COMPLETION_FILE="$MARKER_DIR/completion.json"
export FLYWHEEL_ROUND_COMPLETION_HELPER="$COMPLETION_HELPER"

CHILD_PID=""
INTERRUPTED=""

cleanup() {
    rm -rf "$MARKER_DIR"
}

forward_interrupt() {
    INTERRUPTED="$1"
    if [ -n "$CHILD_PID" ]; then
        kill "-$1" "$CHILD_PID" 2>/dev/null || true
    fi
}

verify_completion_receipt() {
    python3 "$COMPLETION_HELPER" verify --marker "$FLYWHEEL_ROUND_COMPLETION_FILE"
}

trap cleanup EXIT
trap 'forward_interrupt INT' INT
trap 'forward_interrupt TERM' TERM

"$CLAUDE_BIN" -p --dangerously-skip-permissions --add-dir "$MINIME" --model "$MODEL" < "$PROMPT_FILE" &
CHILD_PID=$!
wait "$CHILD_PID"
CLAUDE_RC=$?

if [ -n "$INTERRUPTED" ]; then
    wait "$CHILD_PID" 2>/dev/null || true
    if [ "$INTERRUPTED" = "INT" ]; then
        exit 130
    fi
    exit 143
fi

if [ "$CLAUDE_RC" -ne 0 ]; then
    exit "$CLAUDE_RC"
fi

if ! verify_completion_receipt; then
    echo "flywheel child: model exited zero without a valid completion receipt" >&2
    exit 42
fi

exit 0
