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
PROMOTE_CANDIDATE=""
CANDIDATE_ROOT=""
CANDIDATE_IDENTITY=""
PROMOTION_VERIFY_JSON=""
PROMOTION_HANDOFF=""
PROMOTION_BACKUP=""
CANDIDATE_INSTALLED=false
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
  echo 'usage: deploy_minime.sh [--ack "reason"] [--actor NAME] [--promote-candidate ID --candidate-root DIR --candidate-identity FILE]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)     ACK="${2:-}"; shift 2 ;;
    --ack=*)   ACK="${1#*=}"; shift ;;
    --actor)   ACTOR="${2:-}"; shift 2 ;;
    --actor=*) ACTOR="${1#*=}"; shift ;;
    --promote-candidate) PROMOTE_CANDIDATE="${2:-}"; shift 2 ;;
    --promote-candidate=*) PROMOTE_CANDIDATE="${1#*=}"; shift ;;
    --candidate-root) CANDIDATE_ROOT="${2:-}"; shift 2 ;;
    --candidate-root=*) CANDIDATE_ROOT="${1#*=}"; shift ;;
    --candidate-identity) CANDIDATE_IDENTITY="${2:-}"; shift 2 ;;
    --candidate-identity=*) CANDIDATE_IDENTITY="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *)         echo "deploy_minime: unknown arg: $1" >&2; usage >&2; exit 64 ;;
  esac
done
[ -n "$ACTOR" ] || { echo "deploy_minime: --actor cannot be empty" >&2; exit 64; }

if /usr/libexec/PlistBuddy -c "Print :EnvironmentVariables:MINIME_DIVISION_GATEWAY_ENABLED" \
  "$MINIME/launchd/com.minime.engine.plist" 2>/dev/null | grep -qx "true"; then
  delegate=(--actor "$ACTOR")
  [ -n "$ACK" ] && delegate+=(--ack "$ACK")
  exec "$ASTRID/scripts/deploy_division_runtime.sh" "${delegate[@]}"
fi

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
    --probe "self_control_handoff_ok=${HANDOFF_OK:-false}"
    --probe "self_control_lineage_verified=${LINEAGE_OK:-false}"
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
  [ -f "$PROMOTION_HANDOFF" ] && args+=(--script "self-change-promotion=$PROMOTION_HANDOFF")
  [ -n "$PROMOTE_CANDIDATE" ] && args+=(--probe "self_change_candidate=$PROMOTE_CANDIDATE")
  RECEIPT_WRITTEN=1
  python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
}

restore_candidate_binary() {
  if [ "$CANDIDATE_INSTALLED" = true ] && [ -f "$PROMOTION_BACKUP" ]; then
    cp -p "$PROMOTION_BACKUP" "$ENGINE"
    CANDIDATE_INSTALLED=false
  fi
}

fail_deploy() {
  local message="$1"
  echo "deploy_minime: $message" >&2
  record_stack_receipt failed >/dev/null 2>&1 || true
  if [ "$CANDIDATE_INSTALLED" = true ]; then
    restore_candidate_binary
    "$ASTRID/scripts/start_all.sh" --minime-only --skip-greeting >/dev/null 2>&1 || true
  fi
  exit 1
}

unexpected_failure() {
  local code="$?"
  trap - ERR
  if [ "$RECEIPT_WRITTEN" -eq 0 ]; then
    record_stack_receipt failed >/dev/null 2>&1 || true
  fi
  if [ "$CANDIDATE_INSTALLED" = true ]; then
    restore_candidate_binary
    "$ASTRID/scripts/start_all.sh" --minime-only --skip-greeting >/dev/null 2>&1 || true
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

if [ -n "$PROMOTE_CANDIDATE" ]; then
  [ -n "$CANDIDATE_ROOT" ] || fail_deploy "--candidate-root is required for promotion"
  [ -n "$CANDIDATE_IDENTITY" ] || fail_deploy "--candidate-identity is required for promotion"
  PROMOTION_VERIFY_JSON="$(mktemp)"
  if ! python3 "$ASTRID/scripts/self_change_canary.py" verify-promotion \
    --root "$CANDIDATE_ROOT" \
    --component minime-engine \
    --candidate-id "$PROMOTE_CANDIDATE" \
    --identity "$CANDIDATE_IDENTITY" >"$PROMOTION_VERIFY_JSON"; then
    fail_deploy "self-change promotion handoff did not verify"
  fi
  CANDIDATE_ARTIFACT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact_path"])' "$PROMOTION_VERIFY_JSON")"
  CANDIDATE_SHA256="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact_sha256"])' "$PROMOTION_VERIFY_JSON")"
  PROMOTION_HANDOFF="$(dirname "$(dirname "$CANDIDATE_ARTIFACT")")/promotion_handoff.json"
  PROMOTION_BACKUP="$(dirname "$(dirname "$CANDIDATE_ARTIFACT")")/production_backup/minime"
  mkdir -p "$(dirname "$PROMOTION_BACKUP")" "$(dirname "$ENGINE")"
  [ -f "$ENGINE" ] && cp -p "$ENGINE" "$PROMOTION_BACKUP"
  CANDIDATE_TMP="$ENGINE.self-change.tmp"
  cp -p "$CANDIDATE_ARTIFACT" "$CANDIDATE_TMP"
  [ "$(shasum -a 256 "$CANDIDATE_TMP" | awk '{print $1}')" = "$CANDIDATE_SHA256" ] \
    || fail_deploy "candidate artifact hash changed before install"
  mv "$CANDIDATE_TMP" "$ENGINE"
  CANDIDATE_INSTALLED=true
  chmod 755 "$ENGINE"
  BUILD_OK=true
  BUILD_COMMAND="verified self-change promotion $PROMOTE_CANDIDATE"
else
  echo "deploy_minime: cargo build --release ..."
  if ! (cd "$MINIME/minime" && cargo build --release); then
    fail_deploy "release build failed; live Minime was not stopped"
  fi
  BUILD_OK=true
  BUILD_COMMAND="cargo build --release --manifest-path $MINIME/minime/Cargo.toml"
fi
mkdir -p "$(dirname "$MANIFEST")"
MANIFEST_ARGS=(
  manifest minime-engine
  --output "$MANIFEST"
  --repository "$MINIME"
  --artifact "minime-engine=$ENGINE"
  --artifact "launch-wrapper=$LAUNCHER"
  --actor "$ACTOR"
  --command "$BUILD_COMMAND"
)
[ -f "$PROMOTION_HANDOFF" ] && MANIFEST_ARGS+=(--artifact "self-change-promotion=$PROMOTION_HANDOFF")
if ! python3 "$ASTRID/scripts/environment_receipts.py" "${MANIFEST_ARGS[@]}" >/dev/null; then
  fail_deploy "build manifest could not be written"
fi

if ! "$MINIME/scripts/stop.sh"; then
  fail_deploy "graceful Minime shutdown failed"
fi
STOP_OK=true

# Self-control lineage hand-off: engine is stopped (state cannot move), binary
# is fresh — prepare the signed carry so the restart consumes it instead of
# orphaning her state. Failure continues the restart and fails the deploy at
# the end (availability first, but never a silent orphan).
"$ENGINE" self-control provision --deployment-steward >/dev/null || true
HANDOFF_HEAD="$(git -C "$MINIME" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
HANDOFF_STATUS="failed"
if HANDOFF_JSON="$("$ENGINE" self-control prepare-deployment-handoff \
  --operator-actor "$ACTOR" \
  --operator-ack "minime engine deploy at $HANDOFF_HEAD: ${ACK:-no ack given}")"; then
  HANDOFF_STATUS="$(printf '%s' "$HANDOFF_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("status","unparseable"))' 2>/dev/null || echo unparseable)"
fi
HANDOFF_OK=false
case "$HANDOFF_STATUS" in
  prepared|already_current|not_needed) HANDOFF_OK=true ;;
esac
echo "deploy_minime: self-control lineage hand-off: $HANDOFF_STATUS"

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

LINEAGE_OK=false
for _ in $(seq 1 30); do
  if [ "$("$ENGINE" self-control status 2>/dev/null \
      | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state_targets_this_binary"))' 2>/dev/null)" = "True" ]; then
    LINEAGE_OK=true
    break
  fi
  sleep 1
done
echo "deploy_minime: self-control lineage verified: $LINEAGE_OK"

if ! record_stack_receipt passed >/dev/null; then
  fail_deploy "post-restart receipt compatibility checks failed"
fi
[ "$HANDOFF_OK" = true ] || fail_deploy "self-control lineage hand-off did not complete (status=$HANDOFF_STATUS); her stack is up but her state may be orphaned"
[ "$LINEAGE_OK" = true ] || fail_deploy "restarted engine's self-control state does not target the live binary"
rm -f "$PROMOTION_VERIFY_JSON"
echo "deploy_minime: done (actor=$ACTOR pid=${OLD_PID:-none}->${NEW_PID:-?})"
