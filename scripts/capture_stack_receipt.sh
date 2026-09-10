#!/usr/bin/env bash
# Capture a checked, witness-only identity receipt for the running coupled stack.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
MODEL_REPO="/Users/v/other/neural-triple-reservoir"
SCRIPT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
DOMAIN="gui/$(id -u)"
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"
ACK=""
MODEL_CONTEXT_MANIFEST=""

while [ $# -gt 0 ]; do
  case "$1" in
    --model-context-manifest) MODEL_CONTEXT_MANIFEST="${2:-}"; shift 2 ;;
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
GATEWAY_PID=""
SUPERVISOR_PID=""
PORT_OWNER_PID="$MINIME_PID"
MINIME_MANIFEST="$WORKSPACE/deployment_manifests/minime-engine.json"
DIVISION_ENABLED="$(python3 -c 'import plistlib,sys; print(plistlib.load(open(sys.argv[1], "rb")).get("EnvironmentVariables", {}).get("MINIME_DIVISION_GATEWAY_ENABLED", "false"))' "$HOME/Library/LaunchAgents/com.minime.engine.plist")"
if [ "$DIVISION_ENABLED" = true ]; then
  # This creates a new runtime binding; historical build manifests stay intact.
  MINIME_MANIFEST="$(python3 "$SCRIPT_ROOT/scripts/minime_runtime_binding.py")"
  GATEWAY_PID="$(label_pid com.minime.division-gateway)"
  SUPERVISOR_PID="$(label_pid com.minime.division-supervisor)"
  PORT_OWNER_PID="$GATEWAY_PID"
fi

BRIDGE_PROCESS_OK=false; [ -n "$BRIDGE_PID" ] && kill -0 "$BRIDGE_PID" 2>/dev/null && BRIDGE_PROCESS_OK=true
BRIDGE_BINARY=""
if [ "$BRIDGE_PROCESS_OK" = true ]; then
  BRIDGE_BINARY="$(ps -p "$BRIDGE_PID" -o comm= | sed 's/^[[:space:]]*//')"
  [ -f "$BRIDGE_BINARY" ] || BRIDGE_PROCESS_OK=false
fi
MINIME_PROCESS_OK=false; [ -n "$MINIME_PID" ] && kill -0 "$MINIME_PID" 2>/dev/null && MINIME_PROCESS_OK=true
MODEL_PROCESS_OK=false; [ -n "$MODEL_PID" ] && kill -0 "$MODEL_PID" 2>/dev/null && MODEL_PROCESS_OK=true
PORT_7878_OK=false; [ -n "$PORT_OWNER_PID" ] && lsof -t -nP -iTCP:7878 -sTCP:LISTEN 2>/dev/null | grep -qx "$PORT_OWNER_PID" && PORT_7878_OK=true
PORT_7879_OK=false; [ -n "$PORT_OWNER_PID" ] && lsof -t -nP -iTCP:7879 -sTCP:LISTEN 2>/dev/null | grep -qx "$PORT_OWNER_PID" && PORT_7879_OK=true
LIVEZ_OK=false; [ "$(curl --silent --max-time 2 --output /dev/null --write-out '%{http_code}' http://127.0.0.1:8090/livez 2>/dev/null || true)" = "200" ] && LIVEZ_OK=true
READYZ_OK=false; [ "$(curl --silent --max-time 2 --output /dev/null --write-out '%{http_code}' http://127.0.0.1:8090/readyz 2>/dev/null || true)" = "200" ] && READYZ_OK=true

BRIDGE_MANIFEST="$WORKSPACE/deployment_manifests/spectral-bridge.json"
MODEL_MANIFEST="${MODEL_CONTEXT_MANIFEST:-$WORKSPACE/deployment_manifests/coupled-model.json}"
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
  --binary "minime-current-disk-executable=$MINIME/minime/target/release/minime"
  --binary "model-python=$MODEL_REPO/.venv/bin/python"
  --script "bridge-wrapper=$ASTRID/scripts/build_bridge.sh"
  --script "minime-wrapper=$ASTRID/scripts/deploy_minime.sh"
  --script "model-wrapper=$ASTRID/scripts/restart_coupled_model.sh"
  --manifest "$BRIDGE_MANIFEST"
  --manifest "$MINIME_MANIFEST"
  --context-manifest "$MODEL_MANIFEST"
  --telemetry "$BRIDGE_TELEMETRY"
  --telemetry "$MINIME_TELEMETRY"
)
[ -n "$BRIDGE_BINARY" ] && args+=(--binary "spectral-bridge=$BRIDGE_BINARY")
[ -n "$BRIDGE_PID" ] && args+=(--process "bridge=$BRIDGE_PID")
[ -n "$MINIME_PID" ] && args+=(--process "minime=$MINIME_PID")
[ -n "$MODEL_PID" ] && args+=(--process "model=$MODEL_PID")
[ -n "$GATEWAY_PID" ] && args+=(--process "minime-gateway=$GATEWAY_PID" --launchd-label com.minime.division-gateway)
[ -n "$SUPERVISOR_PID" ] && args+=(--process "minime-supervisor=$SUPERVISOR_PID" --launchd-label com.minime.division-supervisor)

python3 "$SCRIPT_ROOT/scripts/environment_receipts.py" "${args[@]}"
