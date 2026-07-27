#!/usr/bin/env bash
# Build and deploy the dormant sovereign-daughter gateway and supervisor.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
ENGINE="$MINIME/minime/target/release/minime"
RUNTIME_DIR="$MINIME/workspace/division/runtime"
RUNTIME_MANIFEST="$MINIME/workspace/division/runtime-manifest.json"
CEREMONY_LEDGER="$MINIME/workspace/division/ceremony_v1.jsonl"
MINIME_ROOT="$MINIME/workspace/reservoir/minime"
ASTRID_ROOT="$WORKSPACE/reservoir/astrid"
DEPLOYMENT_MANIFEST="$WORKSPACE/deployment_manifests/minime-division-runtime.json"
DOMAIN="gui/$(id -u)"
GATEWAY_LABEL="com.minime.division-gateway"
SUPERVISOR_LABEL="com.minime.division-supervisor"
ENGINE_LABEL="com.minime.engine"

ACK=""
ACTOR="${ASTRID_DEPLOY_ACTOR:-interactive-agent}"

usage() {
  echo 'usage: deploy_division_runtime.sh [--ack "reason"] [--actor NAME]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack) ACK="${2:-}"; shift 2 ;;
    --ack=*) ACK="${1#*=}"; shift ;;
    --actor) ACTOR="${2:-}"; shift 2 ;;
    --actor=*) ACTOR="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "deploy_division_runtime: unknown arg: $1" >&2; usage >&2; exit 64 ;;
  esac
done

label_pid() {
  launchctl print "$DOMAIN/$1" 2>/dev/null |
    awk -F' = ' '/^[[:space:]]*pid = / {print $2; exit}'
}

wait_port_owner() {
  local port="$1" pid="$2" label="$3"
  for _ in $(seq 1 120); do
    if [ -n "$pid" ] && lsof -t -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | grep -qx "$pid"; then
      return 0
    fi
    pid="$(label_pid "$label" || true)"
    sleep 0.25
  done
  return 1
}

sync_plist() {
  local label="$1"
  local source="$MINIME/launchd/$label.plist"
  local installed="$HOME/Library/LaunchAgents/$label.plist"
  plutil -lint "$source" >/dev/null
  install -m 600 "$source" "$installed"
}

bootstrap_label() {
  local label="$1"
  local installed="$HOME/Library/LaunchAgents/$label.plist"
  launchctl bootout "$DOMAIN/$label" >/dev/null 2>&1 || true
  launchctl bootout "$DOMAIN" "$installed" >/dev/null 2>&1 || true
  launchctl bootstrap "$DOMAIN" "$installed"
}

PREFLIGHT=(--component minime --repo "$MINIME")
[ -n "$ACK" ] && PREFLIGHT+=(--ack "$ACK")
python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT[@]}"
(cd "$MINIME/minime" && cargo build --release)
python3 "$ASTRID/scripts/environment_receipts.py" manifest minime-division-runtime \
  --output "$DEPLOYMENT_MANIFEST" \
  --repository "$MINIME" \
  --artifact "minime-engine=$ENGINE" \
  --artifact "gateway-launcher=$MINIME/scripts/launchd_division_gateway.sh" \
  --artifact "supervisor-launcher=$MINIME/scripts/launchd_division_supervisor.sh" \
  --actor "$ACTOR" \
  --command "cargo build --release --manifest-path $MINIME/minime/Cargo.toml" \
  >/dev/null

"$MINIME/scripts/stop.sh"
launchctl bootout "$DOMAIN/$GATEWAY_LABEL" >/dev/null 2>&1 || true
launchctl bootout "$DOMAIN/$SUPERVISOR_LABEL" >/dev/null 2>&1 || true

for port in $(seq 7900 7919); do
  if lsof -t -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "deploy_division_runtime: internal port $port is already occupied after managed services stopped" >&2
    exit 1
  fi
done

for label in \
  "$ENGINE_LABEL" \
  "$GATEWAY_LABEL" \
  "$SUPERVISOR_LABEL" \
  com.minime.division-child-minime \
  com.minime.division-child-astrid
do
  sync_plist "$label"
done

bootstrap_label "$ENGINE_LABEL"
ENGINE_PID="$(label_pid "$ENGINE_LABEL" || true)"
wait_port_owner 7900 "$ENGINE_PID" "$ENGINE_LABEL"
ENGINE_PID="$(label_pid "$ENGINE_LABEL")"
wait_port_owner 7901 "$ENGINE_PID" "$ENGINE_LABEL"
wait_port_owner 7902 "$ENGINE_PID" "$ENGINE_LABEL"

PARENT_GENERATION="$(python3 - <<'PY'
import json
from pathlib import Path
path = Path("/Users/v/other/minime/workspace/division/parent_generation.json")
print(json.loads(path.read_text()).get("generation", 0) if path.is_file() else 0)
PY
)"
PROCESS_START="$(ps -p "$ENGINE_PID" -o lstart= | xargs)"
PROCESS_IDENTITY="$(printf '%s' "$ENGINE_PID:$PROCESS_START" | shasum -a 256 | awk '{print $1}')"
DEPLOYMENT_IDENTITY="$(shasum -a 256 "$DEPLOYMENT_MANIFEST" | awk '{print $1}')"
PLAN_DIGEST="$(printf '%s' "dormant:$DEPLOYMENT_IDENTITY" | shasum -a 256 | awk '{print $1}')"
EXPIRES_AT="$(python3 - <<'PY'
import time
print(int(time.time() * 1000) + 30 * 24 * 60 * 60 * 1000)
PY
)"

python3 "$MINIME/scripts/division_runtime_manifest.py" create \
  --output "$RUNTIME_MANIFEST" \
  --mode dormant \
  --division-id division-dormant-infrastructure \
  --plan-digest "$PLAN_DIGEST" \
  --parent-generation "$PARENT_GENERATION" \
  --candidate-hash unbound \
  --parent-process-identity "$PROCESS_IDENTITY" \
  --parent-deployment-identity "$DEPLOYMENT_IDENTITY" \
  --runtime-dir "$RUNTIME_DIR" \
  --ceremony-ledger "$CEREMONY_LEDGER" \
  --minime-root "$MINIME_ROOT" \
  --astrid-root "$ASTRID_ROOT" \
  --expires-at-unix-ms "$EXPIRES_AT" \
  >/dev/null

bootstrap_label "$GATEWAY_LABEL"
GATEWAY_PID="$(label_pid "$GATEWAY_LABEL" || true)"
for port in 7878 7879 7880 7882 7883; do
  wait_port_owner "$port" "$GATEWAY_PID" "$GATEWAY_LABEL"
  GATEWAY_PID="$(label_pid "$GATEWAY_LABEL")"
done
bootstrap_label "$SUPERVISOR_LABEL"
SUPERVISOR_PID="$(label_pid "$SUPERVISOR_LABEL")"

"$ASTRID/scripts/start_all.sh" --minime-only --skip-greeting

python3 "$ASTRID/scripts/environment_receipts.py" --workspace "$WORKSPACE" \
  record-deploy minime-division-runtime \
  --status passed \
  --actor "$ACTOR" \
  --ack "${ACK:-dormant sovereign daughter runtime deployment}" \
  --new-pid "$GATEWAY_PID" \
  --launchd-label "$ENGINE_LABEL" \
  --launchd-label "$GATEWAY_LABEL" \
  --launchd-label "$SUPERVISOR_LABEL" \
  --probe "parent_internal_ports=true" \
  --probe "gateway_public_ports=true" \
  --probe "supervisor_idle=true" \
  --probe "daughters_dormant=true" \
  --binary "minime-engine=$ENGINE" \
  --manifest "$DEPLOYMENT_MANIFEST" \
  --script "division-wrapper=$ASTRID/scripts/deploy_division_runtime.sh" \
  --process "minime-parent=$ENGINE_PID" \
  --process "division-gateway=$GATEWAY_PID" \
  --process "division-supervisor=$SUPERVISOR_PID" \
  >/dev/null

echo "deploy_division_runtime: parent=$ENGINE_PID gateway=$GATEWAY_PID supervisor=$SUPERVISOR_PID"
