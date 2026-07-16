#!/usr/bin/env bash
# build_bridge.sh - the one supported way to build or restart the live bridge.
#
# The preflight gate prevents a release build from capturing a concurrently
# changing tree. Every attempt writes a witness-only stack receipt, including
# failed preflight, build, manifest, restart, and telemetry checks.
#
# Usage: build_bridge.sh [--ack "reason"] [--actor NAME] [--restart] [--no-build]
set -euo pipefail

ASTRID="/Users/v/other/astrid"
BRIDGE_DIR="$ASTRID/capsules/spectral-bridge"
WORKSPACE="$BRIDGE_DIR/workspace"
BINARY="$BRIDGE_DIR/target/release/spectral-bridge-server"
LAUNCHER="$ASTRID/scripts/launchd_spectral_bridge.sh"
MANIFEST="$WORKSPACE/deployment_manifests/spectral-bridge.json"
TELEMETRY="$WORKSPACE/telemetry_heartbeat_delta_v1.json"
LABEL="com.astrid.spectral-bridge"
DOMAIN="gui/$(id -u)"

ACK=""
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"
DO_BUILD=1
DO_RESTART=0
PREFLIGHT_OK=false
BUILD_OK=false
RESTART_OK=false
LOG_OK=false
TELEMETRY_OK=false
OLD_PID=""
NEW_PID=""
RECEIPT_WRITTEN=0

usage() {
  echo 'usage: build_bridge.sh [--ack "reason"] [--actor NAME] [--restart] [--no-build]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)       ACK="${2:-}"; shift 2 ;;
    --ack=*)     ACK="${1#*=}"; shift ;;
    --actor)     ACTOR="${2:-}"; shift 2 ;;
    --actor=*)   ACTOR="${1#*=}"; shift ;;
    --restart)   DO_RESTART=1; shift ;;
    --no-build)  DO_BUILD=0; shift ;;
    -h|--help)   usage; exit 0 ;;
    *)           echo "build_bridge: unknown arg: $1" >&2; usage >&2; exit 64 ;;
  esac
done

if [ -z "$ACTOR" ]; then
  echo "build_bridge: --actor cannot be empty" >&2
  exit 64
fi

label_pid() {
  local label="${1:-$LABEL}"
  launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/^[[:space:]]*pid = / {print $2; exit}'
}

record_stack_receipt() {
  local requested_status="$1"
  local args=(
    --workspace "$WORKSPACE"
    record-deploy spectral-bridge
    --status "$requested_status"
    --actor "$ACTOR"
    --ack "$ACK"
    --launchd-label "$LABEL"
    --launchd-label "com.minime.engine"
    --launchd-label "com.reservoir.coupled-astrid"
    --probe "preflight=$PREFLIGHT_OK"
    --probe "build=$BUILD_OK"
    --probe "restart=$RESTART_OK"
    --probe "log_clean=$LOG_OK"
    --probe "telemetry_update=$TELEMETRY_OK"
    --binary "spectral-bridge=$BINARY"
    --script "build-wrapper=$ASTRID/scripts/build_bridge.sh"
    --script "launch-wrapper=$LAUNCHER"
  )
  local bridge_pid minime_pid model_pid
  bridge_pid="$(label_pid "$LABEL" || true)"
  minime_pid="$(label_pid com.minime.engine || true)"
  model_pid="$(label_pid com.reservoir.coupled-astrid || true)"
  [ -n "$bridge_pid" ] && args+=(--process "bridge=$bridge_pid")
  [ -n "$minime_pid" ] && args+=(--process "minime=$minime_pid")
  [ -n "$model_pid" ] && args+=(--process "model=$model_pid")
  [ -n "$OLD_PID" ] && args+=(--old-pid "$OLD_PID")
  [ -n "$NEW_PID" ] && args+=(--new-pid "$NEW_PID")
  [ -f "$MANIFEST" ] && args+=(--manifest "$MANIFEST")
  [ -f "$TELEMETRY" ] && args+=(--telemetry "$TELEMETRY")
  RECEIPT_WRITTEN=1
  python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
}

fail_deploy() {
  local message="$1"
  local code="${2:-1}"
  echo "build_bridge: $message" >&2
  record_stack_receipt failed >/dev/null 2>&1 || true
  exit "$code"
}

unexpected_failure() {
  local code="$?"
  trap - ERR
  if [ "$RECEIPT_WRITTEN" -eq 0 ]; then
    record_stack_receipt failed >/dev/null 2>&1 || true
  fi
  exit "$code"
}
trap unexpected_failure ERR

# 1. Preflight gate.
PREFLIGHT_ARGS=(--repo "$ASTRID")
[ -n "$ACK" ] && PREFLIGHT_ARGS+=(--ack "$ACK")
if ! python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT_ARGS[@]}"; then
  fail_deploy "preflight refused; wait for active edits or provide an explicit acknowledgement"
fi
PREFLIGHT_OK=true

HEAD="$(git -C "$ASTRID" rev-parse --short HEAD 2>/dev/null || echo unknown)"

# 2. Build and bind the output to source/protocol identity.
if [ "$DO_BUILD" -eq 1 ]; then
  echo "build_bridge: cargo build --release @ $HEAD ..."
  if ! (cd "$BRIDGE_DIR" && cargo build --release); then
    fail_deploy "cargo build --release failed; live process was not restarted"
  fi
  BUILD_OK=true
  mkdir -p "$(dirname "$MANIFEST")"
  if ! python3 "$ASTRID/scripts/environment_receipts.py" manifest spectral-bridge \
    --output "$MANIFEST" \
    --repository "$ASTRID" \
    --artifact "spectral-bridge=$BINARY" \
    --artifact "launch-wrapper=$LAUNCHER" \
    --actor "$ACTOR" \
    --command "cargo build --release --manifest-path $BRIDGE_DIR/Cargo.toml" \
    >/dev/null; then
    fail_deploy "build manifest could not be written"
  fi
else
  BUILD_OK=true
  if [ ! -f "$MANIFEST" ]; then
    fail_deploy "restart-only requested but no bridge build manifest exists"
  fi
fi

# Build-only is a valid checked capture; restart probes are not applicable.
if [ "$DO_RESTART" -eq 0 ]; then
  RESTART_OK=true
  LOG_OK=true
  TELEMETRY_OK=true
  if ! record_stack_receipt passed >/dev/null; then
    fail_deploy "build receipt compatibility checks failed"
  fi
  echo "build_bridge: done (head=$HEAD actor=$ACTOR built=$DO_BUILD restarted=0)"
  exit 0
fi

# 3. Restart and require a fresh process plus post-restart telemetry.
OLD_PID="$(label_pid "$LABEL" || true)"
OLD_TELEMETRY_MTIME=0
[ -f "$TELEMETRY" ] && OLD_TELEMETRY_MTIME="$(stat -f %m "$TELEMETRY")"
LOG_START=1
[ -f /tmp/bridge.log ] && LOG_START=$(( $(wc -l < /tmp/bridge.log) + 1 ))

if ! launchctl kickstart -k "$DOMAIN/$LABEL"; then
  fail_deploy "launchd restart failed"
fi

for _ in $(seq 1 30); do
  sleep 1
  NEW_PID="$(label_pid "$LABEL" || true)"
  if [ -n "$NEW_PID" ] && [ "$NEW_PID" != "$OLD_PID" ] && kill -0 "$NEW_PID" 2>/dev/null; then
    RESTART_OK=true
    break
  fi
done
[ "$RESTART_OK" = true ] || fail_deploy "restart did not produce a fresh running PID"

for _ in $(seq 1 60); do
  if [ -f "$TELEMETRY" ] && [ "$(stat -f %m "$TELEMETRY")" -gt "$OLD_TELEMETRY_MTIME" ]; then
    TELEMETRY_OK=true
    break
  fi
  sleep 1
done
[ "$TELEMETRY_OK" = true ] || fail_deploy "bridge telemetry did not refresh after restart"

LOG_OK=true
if tail -n +"$LOG_START" /tmp/bridge.log 2>/dev/null | grep -m1 -qiE "panic|fatal|thread '.*panicked"; then
  LOG_OK=false
  fail_deploy "panic or fatal error appeared in the post-restart bridge log"
fi

if ! record_stack_receipt passed >/dev/null; then
  fail_deploy "post-restart receipt compatibility checks failed"
fi

echo "build_bridge: done (head=$HEAD actor=$ACTOR built=$DO_BUILD restarted=1 pid=${OLD_PID:-none}->${NEW_PID:-?})"
