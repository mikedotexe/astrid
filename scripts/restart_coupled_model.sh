#!/usr/bin/env bash
# Gracefully reload the coupled model service and wait for same-port readiness.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MODEL_REPO="/Users/v/other/neural-triple-reservoir"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
SERVER="$MODEL_REPO/coupled_astrid_server.py"
GATEWAY="$MODEL_REPO/coupled_http_gateway.py"
PYTHON_BIN="$MODEL_REPO/.venv/bin/python"
PLIST="$MODEL_REPO/launchd/com.reservoir.coupled-astrid.plist"
INSTALLED_PLIST="$HOME/Library/LaunchAgents/com.reservoir.coupled-astrid.plist"
MANIFEST="$WORKSPACE/deployment_manifests/coupled-model.json"
BRIDGE_TELEMETRY="$WORKSPACE/telemetry_heartbeat_delta_v1.json"
MINIME_TELEMETRY="/Users/v/other/minime/workspace/health.json"
LABEL="com.reservoir.coupled-astrid"
DOMAIN="gui/$(id -u)"

ACK=""
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"
PREFLIGHT_OK=false
MANIFEST_OK=false
GRACEFUL_STOP_OK=false
BOOTSTRAP_OK=false
LIVEZ_OK=false
READYZ_OK=false
TELEMETRY_OK=false
OLD_PID=""
NEW_PID=""
RECEIPT_WRITTEN=0

usage() {
  echo 'usage: restart_coupled_model.sh [--ack "reason"] [--actor NAME]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)     ACK="${2:-}"; shift 2 ;;
    --ack=*)   ACK="${1#*=}"; shift ;;
    --actor)   ACTOR="${2:-}"; shift 2 ;;
    --actor=*) ACTOR="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *)         echo "restart_coupled_model: unknown arg: $1" >&2; usage >&2; exit 64 ;;
  esac
done
[ -n "$ACTOR" ] || { echo "restart_coupled_model: --actor cannot be empty" >&2; exit 64; }

label_pid() {
  local label="${1:-$LABEL}"
  launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/^[[:space:]]*pid = / {print $2; exit}'
}

http_status() {
  curl --silent --show-error --max-time 2 --output /dev/null --write-out '%{http_code}' "$1" 2>/dev/null || true
}

record_stack_receipt() {
  local requested_status="$1"
  local args=(
    --workspace "$WORKSPACE"
    record-deploy coupled-model
    --status "$requested_status"
    --actor "$ACTOR"
    --ack "$ACK"
    --old-pid "${OLD_PID:-}"
    --new-pid "${NEW_PID:-}"
    --launchd-label "$LABEL"
    --launchd-label "com.astrid.spectral-bridge"
    --launchd-label "com.minime.engine"
    --probe "preflight=$PREFLIGHT_OK"
    --probe "manifest=$MANIFEST_OK"
    --probe "graceful_stop=$GRACEFUL_STOP_OK"
    --probe "bootstrap=$BOOTSTRAP_OK"
    --probe "livez=$LIVEZ_OK"
    --probe "readyz=$READYZ_OK"
    --probe "stack_telemetry=$TELEMETRY_OK"
    --binary "python=$PYTHON_BIN"
    --script "server=$SERVER"
    --script "gateway=$GATEWAY"
    --script "launchd-plist=$PLIST"
    --script "restart-wrapper=$ASTRID/scripts/restart_coupled_model.sh"
  )
  local bridge_pid minime_pid model_pid
  bridge_pid="$(label_pid com.astrid.spectral-bridge || true)"
  minime_pid="$(label_pid com.minime.engine || true)"
  model_pid="$(label_pid "$LABEL" || true)"
  [ -n "$bridge_pid" ] && args+=(--process "bridge=$bridge_pid")
  [ -n "$minime_pid" ] && args+=(--process "minime=$minime_pid")
  [ -n "$model_pid" ] && args+=(--process "model=$model_pid")
  [ -f "$MANIFEST" ] && args+=(--manifest "$MANIFEST")
  [ -f "$BRIDGE_TELEMETRY" ] && args+=(--telemetry "$BRIDGE_TELEMETRY")
  [ -f "$MINIME_TELEMETRY" ] && args+=(--telemetry "$MINIME_TELEMETRY")
  RECEIPT_WRITTEN=1
  python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
}

fail_deploy() {
  local message="$1"
  echo "restart_coupled_model: $message" >&2
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

PREFLIGHT_ARGS=(--component model --repo "$MODEL_REPO")
[ -n "$ACK" ] && PREFLIGHT_ARGS+=(--ack "$ACK")
if ! python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT_ARGS[@]}"; then
  fail_deploy "preflight refused; wait for active edits or provide an explicit acknowledgement"
fi
PREFLIGHT_OK=true

mkdir -p "$(dirname "$MANIFEST")" "$(dirname "$INSTALLED_PLIST")"
if ! python3 "$ASTRID/scripts/environment_receipts.py" manifest coupled-model \
  --output "$MANIFEST" \
  --repository "$MODEL_REPO" \
  --artifact "python=$PYTHON_BIN" \
  --artifact "server=$SERVER" \
  --artifact "gateway=$GATEWAY" \
  --artifact "launchd-plist=$PLIST" \
  --actor "$ACTOR" \
  --command "launchd bootstrap $LABEL" \
  >/dev/null; then
  fail_deploy "source manifest could not be written"
fi
MANIFEST_OK=true

OLD_PID="$(label_pid "$LABEL" || true)"
if launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1; then
  if ! launchctl bootout "$DOMAIN/$LABEL"; then
    fail_deploy "launchd bootout failed"
  fi
  for _ in $(seq 1 30); do
    if ! launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1; then
      GRACEFUL_STOP_OK=true
      break
    fi
    sleep 1
  done
else
  GRACEFUL_STOP_OK=true
fi
[ "$GRACEFUL_STOP_OK" = true ] || fail_deploy "model service did not stop within 30 seconds"

cp "$PLIST" "$INSTALLED_PLIST"
if ! launchctl bootstrap "$DOMAIN" "$INSTALLED_PLIST"; then
  fail_deploy "launchd bootstrap failed"
fi
BOOTSTRAP_OK=true

for _ in $(seq 1 30); do
  if [ "$(http_status http://127.0.0.1:8090/livez)" = "200" ]; then
    LIVEZ_OK=true
    break
  fi
  sleep 1
done
[ "$LIVEZ_OK" = true ] || fail_deploy "/livez did not become responsive"

for _ in $(seq 1 1200); do
  if [ "$(http_status http://127.0.0.1:8090/readyz)" = "200" ]; then
    READYZ_OK=true
    break
  fi
  sleep 1
done
[ "$READYZ_OK" = true ] || fail_deploy "/readyz did not report ready"

NEW_PID="$(label_pid "$LABEL" || true)"
if [ -f "$BRIDGE_TELEMETRY" ] && [ -f "$MINIME_TELEMETRY" ]; then
  TELEMETRY_OK=true
fi
[ "$TELEMETRY_OK" = true ] || fail_deploy "stack telemetry evidence is missing"

if ! record_stack_receipt passed >/dev/null; then
  fail_deploy "post-restart receipt compatibility checks failed"
fi
echo "restart_coupled_model: done (actor=$ACTOR pid=${OLD_PID:-none}->${NEW_PID:-?})"
