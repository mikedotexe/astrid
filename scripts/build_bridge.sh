#!/usr/bin/env bash
# build_bridge.sh — the ONE safe way to (re)deploy the live spectral-bridge.
#
# Two agents (Claude/steward-loop + Codex) share this tree and feed one live
# binary. A raw `cargo build --release` from a dirty tree folds the OTHER agent's
# uncommitted code into the live bridge (this session shipped Codex's unreviewed
# llm.rs live more than once). This wrapper gates the build behind
# scripts/deploy_preflight.py so that can't happen silently, then records an
# inspectable deploy receipt. Use this instead of hand-running cargo + kickstart.
#
# Note: `launchctl kickstart -k` only RESTARTS the existing target/release binary;
# it does NOT rebuild. So --restart without a fresh build just re-launches the
# last-built binary (that's what --no-build --restart is for).
#
# Usage: build_bridge.sh [--ack "reason"] [--restart] [--no-build]
#   --ack "reason"  consciously fold in uncommitted bridge source (logged)
#   --restart       kickstart + health-verify after building
#   --no-build      skip the build (restart-only)
set -euo pipefail

ASTRID="/Users/v/other/astrid"
BRIDGE_DIR="$ASTRID/capsules/spectral-bridge"
LABEL="com.astrid.spectral-bridge"

ACK=""
DO_BUILD=1
DO_RESTART=0
while [ $# -gt 0 ]; do
  case "$1" in
    --ack)       ACK="${2:-}"; shift 2 ;;
    --ack=*)     ACK="${1#*=}"; shift ;;
    --restart)   DO_RESTART=1; shift ;;
    --no-build)  DO_BUILD=0; shift ;;
    -h|--help)   echo 'usage: build_bridge.sh [--ack "reason"] [--restart] [--no-build]'; exit 0 ;;
    *)           echo "build_bridge: unknown arg: $1" >&2; exit 64 ;;
  esac
done

SOURCE="claude"; [ -n "${STEWARD_LOOP:-}" ] && SOURCE="steward-loop"

# 1. Preflight gate — foreign mid-edit → abort; dirty bridge source → refuse unless --ack.
PREFLIGHT_ARGS=(--repo "$ASTRID")
[ -n "$ACK" ] && PREFLIGHT_ARGS+=(--ack "$ACK")
if ! python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT_ARGS[@]}"; then
  echo "build_bridge: preflight refused — not building. Commit your work, wait for the other agent, or pass --ack \"reason\"." >&2
  exit 1
fi

HEAD="$(git -C "$ASTRID" rev-parse --short HEAD 2>/dev/null || echo unknown)"
BUILT="no"

# 2. Build (the capture point the gate protects).
if [ "$DO_BUILD" -eq 1 ]; then
  echo "build_bridge: cargo build --release @ $HEAD ..."
  if ! ( cd "$BRIDGE_DIR" && cargo build --release ); then
    echo "build_bridge: cargo build --release FAILED — not restarting." >&2
    exit 1
  fi
  BUILT="yes"
fi

# 3. Restart the live bridge (only restarts the existing binary; build above must precede).
RESTARTED="no"
if [ "$DO_RESTART" -eq 1 ]; then
  OLD_PID="$(pgrep -f spectral-bridge-server || true)"
  launchctl kickstart -k "gui/$(id -u)/$LABEL"
  RESTARTED="yes"
  NEW_PID=""
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    sleep 1
    NEW_PID="$(pgrep -f spectral-bridge-server || true)"
    [ -n "$NEW_PID" ] && [ "$NEW_PID" != "$OLD_PID" ] && break
  done
  echo "build_bridge: restarted (${OLD_PID:-none} -> ${NEW_PID:-?})"
  if tail -30 /tmp/bridge.log 2>/dev/null | grep -m1 -qiE "panic|fatal|thread '"; then
    echo "build_bridge: WARNING — panic/fatal in /tmp/bridge.log after restart; inspect it." >&2
  fi
fi

# 4. Deploy receipt — inspectable, and surfaced to Astrid in welcome_back.txt.
NOTE="build_bridge @ $HEAD"; [ -n "$ACK" ] && NOTE="$NOTE | acked-folds: $ACK"
python3 "$ASTRID/scripts/environment_receipts.py" record deploy \
  --source "$SOURCE" --note "$NOTE" \
  --detail "head=$HEAD" --detail "built=$BUILT" --detail "restarted=$RESTARTED" \
  >/dev/null 2>&1 || true

echo "build_bridge: done (head=$HEAD source=$SOURCE built=$BUILT restarted=$RESTARTED)"
