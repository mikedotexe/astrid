#!/bin/bash
# flywheel_loop_run.sh — durable, unattended driver for the introspection
# source-first flywheel (the Claude successor to the paused Codex automation
# astrid-introspection-source-first-catch-up). Invoked by launchd
# (com.astrid.introspection-flywheel) every 20 minutes.
#
# Each cycle runs ONE bounded round via the sanctioned subprocess adapter:
#   steward_control.py run --actor claude-heartbeat -- flywheel_round_child.sh
# The adapter owns the controller lease (the single cross-steward serializer
# since the advisory mutex was retired), the pre/post projections, heartbeats,
# stop propagation (SIGINT), and the finish outcome (child exit code). The
# child is headless Claude following scripts/flywheel_loop_prompt.txt. A
# nonce-scoped completion receipt prevents a narrative-only zero exit from
# being recorded as a successful round.
#
# Safety: single-flight lock (skip if a prior cycle is running); foreign-
# activity stand-down (tree edited within the last ~3 minutes = a live editor,
# human or external agent — skip this tick); the adapter's begin fails closed
# on any active lease or global pause; outer watchdog SIGINTs the adapter so a
# wedged cycle finishes its lease as failed instead of leaking it. Headless
# rounds NEVER commit, deploy, or touch live controls — commit debt is named
# in each round report and swept in interactive stabilization windows.
#
# Tunables (env): FLYWHEEL_LOOP_MODEL (default opus),
#   FLYWHEEL_LOOP_PROMPT_FILE, FLYWHEEL_LOOP_MAX_SECS (child cap, default 2700),
#   FLYWHEEL_LOOP_OUTER_MAX_SECS (whole-cycle cap, default 7200).
# Disable: launchctl bootout gui/$(id -u)/com.astrid.introspection-flywheel
set -u

# launchd gives a minimal PATH; claude lives in ~/.local/bin.
export PATH="/Users/v/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

ASTRID="/Users/v/other/astrid"

# Optional operator-provided environment (chmod 600, never committed): e.g.
#   export CLAUDE_CODE_OAUTH_TOKEN=...   # from `claude setup-token`
#   export FLYWHEEL_LOOP_MAX_SECS=5400   # budget overrides must land BEFORE
# the assignments below read them (cycle-5 bug: sourcing happened after).
if [ -f "$HOME/.astrid_flywheel_env" ]; then
    # shellcheck disable=SC1091
    . "$HOME/.astrid_flywheel_env"
fi

MAX_SECS="${FLYWHEEL_LOOP_MAX_SECS:-2700}"
OUTER_MAX_SECS="${FLYWHEEL_LOOP_OUTER_MAX_SECS:-7200}"
LOG_DIR="$ASTRID/workspace/logs"
LOG="$LOG_DIR/flywheel_loop.log"
LOCK="/tmp/astrid_flywheel_loop.lock"

mkdir -p "$LOG_DIR"

# Rotate the log if it grows large (keep one previous).
if [ -f "$LOG" ] && [ "$(stat -f%z "$LOG" 2>/dev/null || echo 0)" -gt 5242880 ]; then
    mv -f "$LOG" "$LOG.1"
fi

log() { echo "$(date '+%Y-%m-%dT%H:%M:%S') $*" >> "$LOG"; }

# Single-flight: mkdir is atomic. Rounds run 45-80 min; 20-minute ticks that
# land mid-round are skipped, so the effective cadence is "next round starts
# within 20 minutes of the previous one finishing".
if ! mkdir "$LOCK" 2>/dev/null; then
    log "SKIP — previous flywheel cycle still in progress"
    exit 0
fi
trap 'rmdir "$LOCK" 2>/dev/null' EXIT

# Foreign-activity stand-down: if the tree changed within the activity window
# (~3 min), a live editor (interactive steward or external agent) is present.
# Human-steered work has priority; we resume on a later tick.
if ! FOREIGN_JSON="$(python3 "$ASTRID/scripts/steward_mutex.py" foreign --repo "$ASTRID")"; then
    log "STAND DOWN — live editor in tree: $FOREIGN_JSON"
    exit 0
fi

# Mark this as the loop (interactive-priority session hooks skip themselves).
export STEWARD_LOOP=1


# Auth preflight: a failed credential otherwise costs a full ~40-min
# preprojection before the child dies in seconds. Probe cheaply first; on
# auth failure or hang, skip this tick (free) and retry next tick.
AUTH_PROBE_LOG="$(mktemp /tmp/flywheel_auth_probe.XXXXXX)"
( echo 'Reply with exactly: OK' | claude -p --model haiku > "$AUTH_PROBE_LOG" 2>&1 ) &
PROBE_PID=$!
( sleep 60; kill "$PROBE_PID" 2>/dev/null ) &
PROBE_WATCH=$!
wait "$PROBE_PID"
PROBE_RC=$?
kill "$PROBE_WATCH" 2>/dev/null
if [ "$PROBE_RC" -ne 0 ]; then
    log "STAND DOWN — claude auth/probe unavailable (rc=$PROBE_RC): $(head -c 200 "$AUTH_PROBE_LOG")"
    rm -f "$AUTH_PROBE_LOG"
    exit 0
fi
rm -f "$AUTH_PROBE_LOG"

cd "$ASTRID" || { log "ERROR — cd $ASTRID failed"; exit 1; }
log "===== flywheel cycle START (model=${FLYWHEEL_LOOP_MODEL:-opus}, child cap ${MAX_SECS}s) ====="

# The adapter: begin (lease + preprojection) -> child -> finish (postprojection).
# Outer watchdog SIGINTs the adapter if the whole cycle wedges; the adapter's
# exception path then finishes the lease as failed instead of leaking it.
python3 "$ASTRID/scripts/steward_control.py" run \
    --actor claude-heartbeat \
    --max-secs "$MAX_SECS" \
    -- /bin/bash "$ASTRID/scripts/flywheel_round_child.sh" >> "$LOG" 2>&1 &
ADAPTER_PID=$!
(
    sleep "$OUTER_MAX_SECS"
    kill -INT "$ADAPTER_PID" 2>/dev/null
    sleep 60
    kill -KILL "$ADAPTER_PID" 2>/dev/null
) &
WATCH_PID=$!
disown "$WATCH_PID" 2>/dev/null || true
wait "$ADAPTER_PID"
RC=$?
# Kill the watchdog subshell and any pending sleeps it spawned.
pkill -P "$WATCH_PID" 2>/dev/null
kill "$WATCH_PID" 2>/dev/null

log "===== flywheel cycle END (adapter rc=$RC) ====="
exit 0
