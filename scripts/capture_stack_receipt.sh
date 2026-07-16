#!/usr/bin/env bash
# Capture a checked, witness-only identity receipt for the running coupled stack.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
MODEL_REPO="/Users/v/other/neural-triple-reservoir"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
DOMAIN="gui/$(id -u)"
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"
ACK=""

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)     ACK="${2:-}"; shift 2 ;;
    --ack=*)   ACK="${1#*=}"; shift ;;
    --actor)   ACTOR="${2:-}"; shift 2 ;;
    --actor=*) ACTOR="${1#*=}"; shift ;;
    -h|--help) echo 'usage: capture_stack_receipt.sh [--ack "note"] [--actor NAME]'; exit 0 ;;
    *)         echo "capture_stack_receipt: unknown arg: $1" >&2; exit 64 ;;
  esac
done
[ -n "$ACTOR" ] || { echo "capture_stack_receipt: --actor cannot be empty" >&2; exit 64; }

label_pid() {
  launchctl print "$DOMAIN/$1" 2>/dev/null | awk -F' = ' '/^[[:space:]]*pid = / {print $2; exit}'
}

BRIDGE_PID="$(label_pid com.astrid.spectral-bridge || true)"
MINIME_PID="$(label_pid com.minime.engine || true)"
MODEL_PID="$(label_pid com.reservoir.coupled-astrid || true)"

BRIDGE_PROCESS_OK=false; [ -n "$BRIDGE_PID" ] && kill -0 "$BRIDGE_PID" 2>/dev/null && BRIDGE_PROCESS_OK=true
MINIME_PROCESS_OK=false; [ -n "$MINIME_PID" ] && kill -0 "$MINIME_PID" 2>/dev/null && MINIME_PROCESS_OK=true
MODEL_PROCESS_OK=false; [ -n "$MODEL_PID" ] && kill -0 "$MODEL_PID" 2>/dev/null && MODEL_PROCESS_OK=true
PORT_7878_OK=false; [ -n "$MINIME_PID" ] && lsof -t -nP -iTCP:7878 -sTCP:LISTEN 2>/dev/null | grep -qx "$MINIME_PID" && PORT_7878_OK=true
PORT_7879_OK=false; [ -n "$MINIME_PID" ] && lsof -t -nP -iTCP:7879 -sTCP:LISTEN 2>/dev/null | grep -qx "$MINIME_PID" && PORT_7879_OK=true
LIVEZ_OK=false; [ "$(curl --silent --max-time 2 --output /dev/null --write-out '%{http_code}' http://127.0.0.1:8090/livez 2>/dev/null || true)" = "200" ] && LIVEZ_OK=true
READYZ_OK=false; [ "$(curl --silent --max-time 2 --output /dev/null --write-out '%{http_code}' http://127.0.0.1:8090/readyz 2>/dev/null || true)" = "200" ] && READYZ_OK=true

BRIDGE_MANIFEST="$WORKSPACE/deployment_manifests/spectral-bridge.json"
MINIME_MANIFEST="$WORKSPACE/deployment_manifests/minime-engine.json"
MODEL_MANIFEST="$WORKSPACE/deployment_manifests/coupled-model.json"
BRIDGE_MANIFEST_OK=false; [ -f "$BRIDGE_MANIFEST" ] && BRIDGE_MANIFEST_OK=true
MINIME_MANIFEST_OK=false; [ -f "$MINIME_MANIFEST" ] && MINIME_MANIFEST_OK=true
MODEL_MANIFEST_OK=false; [ -f "$MODEL_MANIFEST" ] && MODEL_MANIFEST_OK=true

BRIDGE_TELEMETRY="$WORKSPACE/telemetry_heartbeat_delta_v1.json"
MINIME_TELEMETRY="$MINIME/workspace/health.json"
TELEMETRY_OK=false; [ -f "$BRIDGE_TELEMETRY" ] && [ -f "$MINIME_TELEMETRY" ] && TELEMETRY_OK=true

STATUS=passed
for check in \
  "$BRIDGE_PROCESS_OK" "$MINIME_PROCESS_OK" "$MODEL_PROCESS_OK" \
  "$PORT_7878_OK" "$PORT_7879_OK" "$LIVEZ_OK" "$READYZ_OK" \
  "$BRIDGE_MANIFEST_OK" "$MINIME_MANIFEST_OK" "$MODEL_MANIFEST_OK" "$TELEMETRY_OK"
do
  [ "$check" = true ] || STATUS=failed
done

args=(
  --workspace "$WORKSPACE"
  record-deploy coupled-stack
  --status "$STATUS"
  --actor "$ACTOR"
  --ack "$ACK"
  --launchd-label com.astrid.spectral-bridge
  --launchd-label com.minime.engine
  --launchd-label com.reservoir.coupled-astrid
  --probe "bridge_process=$BRIDGE_PROCESS_OK"
  --probe "minime_process=$MINIME_PROCESS_OK"
  --probe "model_process=$MODEL_PROCESS_OK"
  --probe "port_7878=$PORT_7878_OK"
  --probe "port_7879=$PORT_7879_OK"
  --probe "livez=$LIVEZ_OK"
  --probe "readyz=$READYZ_OK"
  --probe "bridge_manifest=$BRIDGE_MANIFEST_OK"
  --probe "minime_manifest=$MINIME_MANIFEST_OK"
  --probe "model_manifest=$MODEL_MANIFEST_OK"
  --probe "telemetry=$TELEMETRY_OK"
  --binary "spectral-bridge=$ASTRID/capsules/spectral-bridge/target/release/spectral-bridge-server"
  --binary "minime-engine=$MINIME/minime/target/release/minime"
  --binary "model-python=$MODEL_REPO/.venv/bin/python"
  --script "bridge-wrapper=$ASTRID/scripts/build_bridge.sh"
  --script "minime-wrapper=$ASTRID/scripts/deploy_minime.sh"
  --script "model-wrapper=$ASTRID/scripts/restart_coupled_model.sh"
  --manifest "$BRIDGE_MANIFEST"
  --manifest "$MINIME_MANIFEST"
  --manifest "$MODEL_MANIFEST"
  --telemetry "$BRIDGE_TELEMETRY"
  --telemetry "$MINIME_TELEMETRY"
)
[ -n "$BRIDGE_PID" ] && args+=(--process "bridge=$BRIDGE_PID")
[ -n "$MINIME_PID" ] && args+=(--process "minime=$MINIME_PID")
[ -n "$MODEL_PID" ] && args+=(--process "model=$MODEL_PID")

python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
