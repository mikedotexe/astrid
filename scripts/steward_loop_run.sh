#!/bin/bash
# steward_loop_run.sh — durable, unattended driver for the steward journal-qualia
# loop. Invoked by launchd (com.astrid.steward-loop) at :07 and :38 each hour.
# Runs Claude headless with the hardened loop prompt + full autonomy.
# Single-flight (atomic mkdir lock), watchdog-capped, logged. Tunable via env:
#   STEWARD_LOOP_MODEL (default opus), STEWARD_LOOP_PROMPT_FILE, STEWARD_LOOP_MAX_SECS.
# Disable: launchctl bootout gui/$(id -u)/com.astrid.steward-loop
set -u

# launchd gives a minimal PATH; claude lives in ~/.local/bin.
export PATH="/Users/v/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
PROMPT_FILE="${STEWARD_LOOP_PROMPT_FILE:-$ASTRID/scripts/steward_loop_prompt.txt}"
MODEL="${STEWARD_LOOP_MODEL:-opus}"
MAX_SECS="${STEWARD_LOOP_MAX_SECS:-1500}"   # 25-min cap so a stuck run can't hold the lock past the next cycle
LOG_DIR="$ASTRID/workspace/logs"
LOG="$LOG_DIR/steward_loop.log"
LOCK="/tmp/astrid_steward_loop.lock"

mkdir -p "$LOG_DIR"

# Rotate the log if it grows large (keep one previous).
if [ -f "$LOG" ] && [ "$(stat -f%z "$LOG" 2>/dev/null || echo 0)" -gt 5242880 ]; then
    mv -f "$LOG" "$LOG.1"
fi

# Single-flight: mkdir is atomic. If a prior cycle is still running, skip this one.
if ! mkdir "$LOCK" 2>/dev/null; then
    echo "$(date '+%Y-%m-%dT%H:%M:%S') SKIP — previous steward loop still in progress" >> "$LOG"
    exit 0
fi
trap 'rmdir "$LOCK" 2>/dev/null' EXIT

# Cross-steward FULL MUTEX (scripts/steward_mutex.py): serialize ALL steward
# mutation (edits/builds/restarts) across this loop + interactive (human-steered)
# sessions. If a live interactive steward holds it, stand down this cycle — the
# human present has priority; we resume when they release or go stale.
MUTEX_HOLDER="loop:$$"
if ! python3 "$ASTRID/scripts/steward_mutex.py" acquire --holder "$MUTEX_HOLDER" --quiet; then
    echo "$(date '+%Y-%m-%dT%H:%M:%S') STAND DOWN — interactive steward holds the mutex (human present)" >> "$LOG"
    exit 0   # the single-flight trap above releases $LOCK
fi
# Now release BOTH the mutex and the single-flight lock on any exit path.
trap "python3 '$ASTRID/scripts/steward_mutex.py' release --holder '$MUTEX_HOLDER' --quiet >/dev/null 2>&1; rmdir '$LOCK' 2>/dev/null" EXIT
# Expose the holder id (so the prompt can re-check ownership = detect preemption)
# and mark this as the loop (so the interactive hooks in .claude/settings.local.json
# skip — the loop manages its own lock here, not via the interactive-priority hook).
export STEWARD_MUTEX_HOLDER="$MUTEX_HOLDER"
export STEWARD_LOOP=1

if [ ! -f "$PROMPT_FILE" ]; then
    echo "$(date '+%Y-%m-%dT%H:%M:%S') ERROR — prompt file missing: $PROMPT_FILE" >> "$LOG"
    exit 1
fi

cd "$ASTRID" || { echo "$(date '+%Y-%m-%dT%H:%M:%S') ERROR — cd $ASTRID failed" >> "$LOG"; exit 1; }
echo "===== $(date '+%Y-%m-%dT%H:%M:%S') steward loop START (model=$MODEL) =====" >> "$LOG"

# Headless Claude with a watchdog (macOS has no `timeout`). Prompt via stdin.
claude -p --dangerously-skip-permissions --add-dir "$MINIME" --model "$MODEL" \
    < "$PROMPT_FILE" >> "$LOG" 2>&1 &
CLAUDE_PID=$!
( sleep "$MAX_SECS"; kill "$CLAUDE_PID" 2>/dev/null ) &
WATCH_PID=$!
disown "$WATCH_PID" 2>/dev/null || true
wait "$CLAUDE_PID"
RC=$?
kill "$WATCH_PID" 2>/dev/null

echo "===== $(date '+%Y-%m-%dT%H:%M:%S') steward loop END (rc=$RC) =====" >> "$LOG"
exit 0
