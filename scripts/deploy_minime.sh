#!/usr/bin/env bash
# Build and gracefully replace the launchd-managed Minime engine and companions.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
ENGINE="$MINIME/minime/target/release/minime"
LAUNCHER="$MINIME/scripts/launchd_minime_engine.sh"
MANIFEST="$WORKSPACE/deployment_manifests/minime-engine.json"
TELEMETRY="$MINIME/workspace/health.json"
LABEL="com.minime.engine"
DOMAIN="gui/$(id -u)"

ACK=""
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"
PREFLIGHT_OK=false
BUILD_OK=false
STOP_OK=false
START_OK=false
PORT_7878_OK=false
PORT_7879_OK=false
TELEMETRY_OK=false
OLD_PID=""
NEW_PID=""
RECEIPT_WRITTEN=0

usage() {
  echo 'usage: deploy_minime.sh [--ack "reason"] [--actor NAME]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)     ACK="${2:-}"; shift 2 ;;
    --ack=*)   ACK="${1#*=}"; shift ;;
    --actor)   ACTOR="${2:-}"; shift 2 ;;
    --actor=*) ACTOR="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *)         echo "deploy_minime: unknown arg: $1" >&2; usage >&2; exit 64 ;;
  esac
done
[ -n "$ACTOR" ] || { echo "deploy_minime: --actor cannot be empty" >&2; exit 64; }

label_pid() {
  local label="${1:-$LABEL}"
  launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/^[[:space:]]*pid = / {print $2; exit}'
}

port_owned_by_new_pid() {
  local port="$1"
  [ -n "$NEW_PID" ] && lsof -t -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | grep -qx "$NEW_PID"
}

record_stack_receipt() {
  local requested_status="$1"
  local args=(
    --workspace "$WORKSPACE"
    record-deploy minime-engine
    --status "$requested_status"
    --actor "$ACTOR"
    --ack "$ACK"
    --old-pid "${OLD_PID:-}"
    --new-pid "${NEW_PID:-}"
    --launchd-label "$LABEL"
    --launchd-label "com.minime.host-sensory"
    --launchd-label "com.minime.camera-client"
    --launchd-label "com.minime.mic-to-sensory"
    --launchd-label "com.minime.visual-frame-service"
    --launchd-label "com.minime.autonomous-agent"
    --launchd-label "com.astrid.spectral-bridge"
    --launchd-label "com.reservoir.coupled-astrid"
    --probe "preflight=$PREFLIGHT_OK"
    --probe "build=$BUILD_OK"
    --probe "graceful_stop=$STOP_OK"
    --probe "launchd_restore=$START_OK"
    --probe "port_7878=$PORT_7878_OK"
    --probe "port_7879=$PORT_7879_OK"
    --probe "telemetry_update=$TELEMETRY_OK"
    --binary "minime-engine=$ENGINE"
    --script "deploy-wrapper=$ASTRID/scripts/deploy_minime.sh"
    --script "launch-wrapper=$LAUNCHER"
  )
  local bridge_pid minime_pid model_pid
  bridge_pid="$(label_pid com.astrid.spectral-bridge || true)"
  minime_pid="$(label_pid "$LABEL" || true)"
  model_pid="$(label_pid com.reservoir.coupled-astrid || true)"
  [ -n "$bridge_pid" ] && args+=(--process "bridge=$bridge_pid")
  [ -n "$minime_pid" ] && args+=(--process "minime=$minime_pid")
  [ -n "$model_pid" ] && args+=(--process "model=$model_pid")
  [ -f "$MANIFEST" ] && args+=(--manifest "$MANIFEST")
  [ -f "$TELEMETRY" ] && args+=(--telemetry "$TELEMETRY")
  RECEIPT_WRITTEN=1
  python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
}

fail_deploy() {
  local message="$1"
  echo "deploy_minime: $message" >&2
  record_stack_receipt failed >/dev/null 2>&1 || true
  exit 1
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

PREFLIGHT_ARGS=(--component minime --repo "$MINIME")
[ -n "$ACK" ] && PREFLIGHT_ARGS+=(--ack "$ACK")
if ! python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT_ARGS[@]}"; then
  fail_deploy "preflight refused; wait for active edits or provide an explicit acknowledgement"
fi
PREFLIGHT_OK=true

OLD_PID="$(label_pid "$LABEL" || true)"
OLD_TELEMETRY_MTIME=0
[ -f "$TELEMETRY" ] && OLD_TELEMETRY_MTIME="$(stat -f %m "$TELEMETRY")"

echo "deploy_minime: cargo build --release ..."
if ! (cd "$MINIME/minime" && cargo build --release); then
  fail_deploy "release build failed; live Minime was not stopped"
fi
BUILD_OK=true
mkdir -p "$(dirname "$MANIFEST")"
if ! python3 "$ASTRID/scripts/environment_receipts.py" manifest minime-engine \
  --output "$MANIFEST" \
  --repository "$MINIME" \
  --artifact "minime-engine=$ENGINE" \
  --artifact "launch-wrapper=$LAUNCHER" \
  --actor "$ACTOR" \
  --command "cargo build --release --manifest-path $MINIME/minime/Cargo.toml" \
  >/dev/null; then
  fail_deploy "build manifest could not be written"
fi

if ! "$MINIME/scripts/stop.sh"; then
  fail_deploy "graceful Minime shutdown failed"
fi
STOP_OK=true

if ! "$ASTRID/scripts/start_all.sh" --minime-only --skip-greeting; then
  fail_deploy "start_all.sh --minime-only failed"
fi
START_OK=true
NEW_PID="$(label_pid "$LABEL" || true)"

if port_owned_by_new_pid 7878; then PORT_7878_OK=true; fi
if port_owned_by_new_pid 7879; then PORT_7879_OK=true; fi
[ "$PORT_7878_OK" = true ] || fail_deploy "new engine PID does not own telemetry port 7878"
[ "$PORT_7879_OK" = true ] || fail_deploy "new engine PID does not own sensory port 7879"

for _ in $(seq 1 90); do
  if [ -f "$TELEMETRY" ] && [ "$(stat -f %m "$TELEMETRY")" -gt "$OLD_TELEMETRY_MTIME" ]; then
    TELEMETRY_OK=true
    break
  fi
  sleep 1
done
[ "$TELEMETRY_OK" = true ] || fail_deploy "Minime telemetry did not refresh after restore"

if ! record_stack_receipt passed >/dev/null; then
  fail_deploy "post-restart receipt compatibility checks failed"
fi
echo "deploy_minime: done (actor=$ACTOR pid=${OLD_PID:-none}->${NEW_PID:-?})"
