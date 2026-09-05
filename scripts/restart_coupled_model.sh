#!/usr/bin/env bash
# Gracefully reload the coupled model service and wait for same-port readiness.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
MODEL_REPO="/Users/v/other/neural-triple-reservoir"
WORKSPACE="$ASTRID/capsules/spectral-bridge/workspace"
SERVER="$MODEL_REPO/coupled_astrid_server.py"
GATEWAY="$MODEL_REPO/coupled_http_gateway.py"
PROCESSOR="$MODEL_REPO/mlx_reservoir.py"
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
MANIFEST_PUBLISHED=false
GRACEFUL_STOP_OK=false
RELOAD_OK=false
LIVEZ_OK=false
READYZ_OK=false
TELEMETRY_OK=false
OLD_PID=""
OLD_STARTED_AT=""
NEW_PID=""
RELOAD_RECEIPT=""
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
    --old-started-at "$OLD_STARTED_AT"
    --new-pid "${NEW_PID:-}"
    --launchd-label "$LABEL"
    --launchd-label "com.astrid.spectral-bridge"
    --launchd-label "com.minime.engine"
    --probe "preflight=$PREFLIGHT_OK"
    --probe "manifest=$MANIFEST_OK"
    --probe "manifest_published_after_readiness=$MANIFEST_PUBLISHED"
    --probe "graceful_stop=$GRACEFUL_STOP_OK"
    --probe "keepalive_reload=$RELOAD_OK"
    --probe "livez=$LIVEZ_OK"
    --probe "readyz=$READYZ_OK"
    --probe "stack_telemetry=$TELEMETRY_OK"
    --binary "python=$PYTHON_BIN"
    --script "server=$SERVER"
    --script "gateway=$GATEWAY"
    --script "logit-processor=$PROCESSOR"
    --script "launchd-plist=$PLIST"
    --script "restart-wrapper=$ASTRID/scripts/restart_coupled_model.sh"
    --script "reload-helper=$ASTRID/scripts/graceful_model_reload.py"
  )
  local bridge_pid minime_pid model_pid
  bridge_pid="$(label_pid com.astrid.spectral-bridge || true)"
  minime_pid="$(label_pid com.minime.engine || true)"
  model_pid="$(label_pid "$LABEL" || true)"
  [ -n "$bridge_pid" ] && args+=(--process "bridge=$bridge_pid")
  [ -n "$minime_pid" ] && args+=(--process "minime=$minime_pid")
  [ -n "$model_pid" ] && args+=(--process "model=$model_pid")
  [ -f "$RELOAD_RECEIPT" ] && args+=(--script "graceful-reload=$RELOAD_RECEIPT")
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

# This reload preserves the already-loaded launchd configuration. A configuration
# migration needs its own reviewed path, not bootout's forced-exit deadline.
if ! cmp -s "$PLIST" "$INSTALLED_PLIST"; then
  fail_deploy "source/installed plist mismatch; refusing an implicit configuration migration"
fi
OLD_PID="$(label_pid "$LABEL" || true)"
[ -n "$OLD_PID" ] || fail_deploy "no running model PID to drain"

# A local model readiness check cannot waive a failed stack identity/topology.
if ! bash "$ASTRID/scripts/capture_stack_receipt.sh" --actor "$ACTOR" --ack "$ACK; pre-model-reload inventory"; then
  fail_deploy "pre-reload stack identity/topology receipt failed; no signal sent"
fi
if ! python3 "$ASTRID/scripts/deploy_preflight.py" "${PREFLIGHT_ARGS[@]}"; then
  fail_deploy "source changed while capturing stack identity; no signal sent"
fi

mkdir -p "$(dirname "$MANIFEST")" "$(dirname "$INSTALLED_PLIST")"
RELOAD_DIR="$(mktemp -d "$(dirname "$MANIFEST")/coupled-model-reload.XXXXXX")"
RELOAD_RECEIPT="$RELOAD_DIR/drain.json"
CANDIDATE_MANIFEST="$RELOAD_DIR/candidate-manifest.json"
[ ! -f "$MANIFEST" ] || cp -p "$MANIFEST" "$RELOAD_DIR/previous-manifest.json"
if ! python3 "$ASTRID/scripts/environment_receipts.py" manifest coupled-model \
  --output "$CANDIDATE_MANIFEST" \
  --repository "$MODEL_REPO" \
  --artifact "python=$PYTHON_BIN" \
  --artifact "server=$SERVER" \
  --artifact "gateway=$GATEWAY" \
  --artifact "logit-processor=$PROCESSOR" \
  --artifact "launchd-plist=$PLIST" \
  --actor "$ACTOR" \
  --command "verified launchd SIGTERM drain and KeepAlive reload $LABEL" \
  >/dev/null; then
  fail_deploy "source manifest could not be written"
fi
MANIFEST_OK=true

if ! python3 "$ASTRID/scripts/graceful_model_reload.py" --expected-pid "$OLD_PID" \
  >"$RELOAD_RECEIPT"; then
  fail_deploy "graceful model drain/reload failed; no forced termination attempted"
fi
GRACEFUL_STOP_OK=true
NEW_PID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["new_pid"])' "$RELOAD_RECEIPT")"
OLD_STARTED_AT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["old_started_at"])' "$RELOAD_RECEIPT")"
RELOAD_OK=true

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

[ "$(label_pid "$LABEL" || true)" = "$NEW_PID" ] || fail_deploy "model PID changed during readiness checks"
if ! lsof -t -nP -iTCP:8090 -sTCP:LISTEN 2>/dev/null | grep -qx "$NEW_PID"; then
  fail_deploy "ready endpoint is not owned by the verified replacement PID"
fi
if [ -f "$BRIDGE_TELEMETRY" ] && [ -f "$MINIME_TELEMETRY" ]; then
  TELEMETRY_OK=true
fi
[ "$TELEMETRY_OK" = true ] || fail_deploy "stack telemetry evidence is missing"

# Keep the old published manifest until the verified replacement is ready.
cp "$CANDIDATE_MANIFEST" "$RELOAD_DIR/publish-manifest.json"
mv "$RELOAD_DIR/publish-manifest.json" "$MANIFEST"
MANIFEST_PUBLISHED=true

if ! record_stack_receipt passed >/dev/null; then
  fail_deploy "post-restart receipt compatibility checks failed"
fi
if ! bash "$ASTRID/scripts/capture_stack_receipt.sh" --actor "$ACTOR" --ack "$ACK; post-model-reload inventory"; then
  fail_deploy "post-reload full stack receipt failed; preserve this alignment debt"
fi
echo "restart_coupled_model: done (actor=$ACTOR pid=${OLD_PID:-none}->${NEW_PID:-?})"
