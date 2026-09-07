#!/usr/bin/env bash
# build_bridge.sh - the one supported way to build or restart the live bridge.
#
# The preflight gate prevents a release build from capturing a concurrently
# changing tree. Every attempt writes a witness-only stack receipt, including
# failed preflight, build, manifest, restart, and telemetry checks.
#
# Usage: build_bridge.sh [--ack "reason"] [--actor NAME] [--restart] [--no-build]
#   [--promote-candidate ID --candidate-root DIR --candidate-identity FILE]
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
DO_DRAIN_ONLY=0
STAGE_DIR=""
ACTIVATE_STAGE=""
RESUME_VERIFICATION=""
EXPECTED_PID=""
LEGACY_STOP_ACK=""
ACTIVATION_TIMEOUT=600
SCRIPT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFLIGHT_OK=false
BUILD_OK=false
RESTART_OK=false
LOG_OK=false
TELEMETRY_OK=false
OLD_PID=""
OLD_STARTED_AT=""
OLD_COMMAND=""
OLD_CAPTURED_AT=""
NEW_PID=""
RECEIPT_WRITTEN=0
PROMOTE_CANDIDATE=""
CANDIDATE_ROOT=""
CANDIDATE_IDENTITY=""
PROMOTION_VERIFY_JSON=""
PROMOTION_HANDOFF=""
PROMOTION_BACKUP=""
CANDIDATE_INSTALLED=false

usage() {
  echo 'usage: build_bridge.sh [--ack "reason"] [--actor NAME] [--stage-dir DIR | --activate-stage DIR --expected-pid PID [--legacy-stop-ack "explicit approval" | --resume-verification TRANSACTION] [--timeout-secs N] | --drain-only | --restart] [--no-build] [--promote-candidate ID --candidate-root DIR --candidate-identity FILE]'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ack)       ACK="${2:-}"; shift 2 ;;
    --ack=*)     ACK="${1#*=}"; shift ;;
    --actor)     ACTOR="${2:-}"; shift 2 ;;
    --actor=*)   ACTOR="${1#*=}"; shift ;;
    --restart)   DO_RESTART=1; shift ;;
    --drain-only) DO_DRAIN_ONLY=1; shift ;;
    --stage-dir) STAGE_DIR="${2:-}"; shift 2 ;;
    --activate-stage) ACTIVATE_STAGE="${2:-}"; shift 2 ;;
    --expected-pid) EXPECTED_PID="${2:-}"; shift 2 ;;
    --resume-verification)
      [ -n "${2:-}" ] && [[ "${2:-}" != --* ]] \
        || { echo "build_bridge: --resume-verification requires a transaction directory" >&2; exit 64; }
      RESUME_VERIFICATION="$2"; shift 2 ;;
    --legacy-stop-ack) LEGACY_STOP_ACK="${2:-}"; shift 2 ;;
    --timeout-secs) ACTIVATION_TIMEOUT="${2:-}"; shift 2 ;;
    --no-build)  DO_BUILD=0; shift ;;
    --promote-candidate) PROMOTE_CANDIDATE="${2:-}"; shift 2 ;;
    --promote-candidate=*) PROMOTE_CANDIDATE="${1#*=}"; shift ;;
    --candidate-root) CANDIDATE_ROOT="${2:-}"; shift 2 ;;
    --candidate-root=*) CANDIDATE_ROOT="${1#*=}"; shift ;;
    --candidate-identity) CANDIDATE_IDENTITY="${2:-}"; shift 2 ;;
    --candidate-identity=*) CANDIDATE_IDENTITY="${1#*=}"; shift ;;
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

# A drain-only request never builds, publishes a manifest, or restarts a job.
# The old live binary has no lifecycle acknowledgement and is refused before
# any signal. This candidate does not enable the legacy force-restart route.
if [ -n "$ACTIVATE_STAGE" ]; then
  [ "$DO_RESTART" -eq 0 ] && [ "$DO_DRAIN_ONLY" -eq 0 ] && [ "$DO_BUILD" -eq 1 ] && [ -z "$STAGE_DIR" ] && [ -z "$PROMOTE_CANDIDATE" ] && [ -z "$CANDIDATE_ROOT" ] && [ -z "$CANDIDATE_IDENTITY" ] && [ -n "$ACK" ] && [ -n "$EXPECTED_PID" ] \
    || { echo "build_bridge: activation requires --ack/--expected-pid and cannot accompany other build/restart modes" >&2; exit 64; }
  if [ -n "$RESUME_VERIFICATION" ] && [ -n "$LEGACY_STOP_ACK" ]; then
    echo "build_bridge: verification-only recovery cannot accompany legacy stop acknowledgement" >&2
    exit 64
  fi
  # Bash 3 treats an empty array as unset under nounset; keep required arguments
  # in the array so ordinary activation does not require a recovery option.
  ACTIVATION_ARGS=(
    --stage-dir "$ACTIVATE_STAGE" --expected-pid "$EXPECTED_PID" --actor "$ACTOR" --ack "$ACK"
    --legacy-stop-ack "$LEGACY_STOP_ACK" --timeout-secs "$ACTIVATION_TIMEOUT"
  )
  [ -z "$RESUME_VERIFICATION" ] || ACTIVATION_ARGS+=(--resume-verification "$RESUME_VERIFICATION")
  python3 -B "$SCRIPT_ROOT/scripts/deploy_preflight.py" --repo "$SCRIPT_ROOT" --ack "$ACK"
  if [ "$SCRIPT_ROOT" != "$ASTRID" ]; then
    python3 -B "$SCRIPT_ROOT/scripts/deploy_preflight.py" --repo "$ASTRID" --ack "$ACK"
  fi
  exec env ASTRID_SANCTIONED_BRIDGE_ACTIVATION=1 python3 -B "$SCRIPT_ROOT/scripts/bridge_activate.py" \
    "${ACTIVATION_ARGS[@]}"
fi
if [ -n "$EXPECTED_PID" ] || [ -n "$LEGACY_STOP_ACK" ] || [ -n "$RESUME_VERIFICATION" ] || [ "$ACTIVATION_TIMEOUT" != 600 ]; then
  echo "build_bridge: activation options require --activate-stage" >&2
  exit 64
fi
if [ -n "$STAGE_DIR" ]; then
  [ "$DO_RESTART" -eq 0 ] && [ "$DO_DRAIN_ONLY" -eq 0 ] && [ "$DO_BUILD" -eq 1 ] && [ -z "$PROMOTE_CANDIDATE" ] && [ -n "$ACK" ] \
    || { echo "build_bridge: staging requires --ack and cannot accompany restart/drain/no-build/promotion" >&2; exit 64; }
  python3 -B "$SCRIPT_ROOT/scripts/deploy_preflight.py" --repo "$SCRIPT_ROOT" --ack "$ACK"
  exec env ASTRID_SANCTIONED_BRIDGE_STAGE=1 python3 -B "$SCRIPT_ROOT/scripts/bridge_stage.py" build \
    --source-root "$SCRIPT_ROOT" --stage-dir "$STAGE_DIR" --actor "$ACTOR" --ack "$ACK"
fi
if [ "$DO_DRAIN_ONLY" -eq 1 ]; then
  [ "$DO_RESTART" -eq 0 ] && [ -z "$PROMOTE_CANDIDATE" ] && [ -n "$ACK" ] \
    || { echo "build_bridge: drain-only requires --ack and cannot accompany restart/promotion" >&2; exit 64; }
  python3 "$ASTRID/scripts/deploy_preflight.py" --repo "$ASTRID" --ack "$ACK"
  DRAIN_PID="$(label_pid || true)"
  [ -n "$DRAIN_PID" ] || { echo "build_bridge: no live bridge PID" >&2; exit 1; }
  exec python3 -B "$SCRIPT_ROOT/scripts/bridge_drain.py" \
    --pid "$DRAIN_PID" --binary "$BINARY" \
    --status-dir "$ASTRID/.runtime/bridge-lifecycle" --request --ack "$ACK"
fi

if [ "$SCRIPT_ROOT" != "$ASTRID" ]; then
  echo "build_bridge: candidate worktree cannot build or replace the canonical live artifact; deployment reconciliation is still required" >&2
  exit 64
fi
if [ "$DO_RESTART" -eq 1 ]; then
  echo "build_bridge: legacy forced replacement is disabled in this candidate; reviewed manifest publication and signed handoff integration are required" >&2
  exit 64
fi

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
  [ -n "$OLD_STARTED_AT" ] && args+=(--old-started-at "$OLD_STARTED_AT")
  [ -n "$OLD_COMMAND" ] && args+=(--old-command "$OLD_COMMAND")
  [ -n "$OLD_CAPTURED_AT" ] && args+=(--old-captured-at "$OLD_CAPTURED_AT")
  [ -n "$NEW_PID" ] && args+=(--new-pid "$NEW_PID")
  [ -f "$MANIFEST" ] && args+=(--manifest "$MANIFEST")
  [ -f "$TELEMETRY" ] && args+=(--telemetry "$TELEMETRY")
  [ -f "$PROMOTION_HANDOFF" ] && args+=(--script "self-change-promotion=$PROMOTION_HANDOFF")
  [ -n "$PROMOTE_CANDIDATE" ] && args+=(--probe "self_change_candidate=$PROMOTE_CANDIDATE")
  # record-deploy probes must be NAME=BOOL (b3491d14b7); the 4-state hand-off
  # string killed the first post-restart receipt of the 2026-09-02 deploy.
  if [ -n "${SELF_CONTROL_HANDOFF:-}" ]; then
    local handoff_ok=false
    case "$SELF_CONTROL_HANDOFF" in
      prepared|already_current|skipped) handoff_ok=true ;;
    esac
    args+=(--probe "self_control_handoff_ok=$handoff_ok")
  fi
  RECEIPT_WRITTEN=1
  python3 "$ASTRID/scripts/environment_receipts.py" "${args[@]}"
}

restore_candidate_binary() {
  if [ "$CANDIDATE_INSTALLED" = true ] && [ -f "$PROMOTION_BACKUP" ]; then
    cp -p "$PROMOTION_BACKUP" "$BINARY"
    CANDIDATE_INSTALLED=false
  fi
}

fail_deploy() {
  local message="$1"
  local code="${2:-1}"
  echo "build_bridge: $message" >&2
  record_stack_receipt failed >/dev/null 2>&1 || true
  if [ "$CANDIDATE_INSTALLED" = true ]; then
    restore_candidate_binary
    launchctl kickstart -k "$DOMAIN/$LABEL" >/dev/null 2>&1 || true
  fi
  exit "$code"
}

unexpected_failure() {
  local code="$?"
  trap - ERR
  if [ "$RECEIPT_WRITTEN" -eq 0 ]; then
    record_stack_receipt failed >/dev/null 2>&1 || true
  fi
  if [ "$CANDIDATE_INSTALLED" = true ]; then
    restore_candidate_binary
    launchctl kickstart -k "$DOMAIN/$LABEL" >/dev/null 2>&1 || true
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
if [ -n "$PROMOTE_CANDIDATE" ]; then
  [ "$DO_RESTART" -eq 1 ] || fail_deploy "self-change promotion requires --restart" 64
  [ -n "$CANDIDATE_ROOT" ] || fail_deploy "--candidate-root is required for promotion" 64
  [ -n "$CANDIDATE_IDENTITY" ] || fail_deploy "--candidate-identity is required for promotion" 64
  PROMOTION_VERIFY_JSON="$(mktemp)"
  if ! python3 "$ASTRID/scripts/self_change_canary.py" verify-promotion \
    --root "$CANDIDATE_ROOT" \
    --component spectral-bridge \
    --candidate-id "$PROMOTE_CANDIDATE" \
    --identity "$CANDIDATE_IDENTITY" >"$PROMOTION_VERIFY_JSON"; then
    fail_deploy "self-change promotion handoff did not verify"
  fi
  CANDIDATE_ARTIFACT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact_path"])' "$PROMOTION_VERIFY_JSON")"
  CANDIDATE_SHA256="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact_sha256"])' "$PROMOTION_VERIFY_JSON")"
  PROMOTION_HANDOFF="$(dirname "$(dirname "$CANDIDATE_ARTIFACT")")/promotion_handoff.json"
  PROMOTION_BACKUP="$(dirname "$(dirname "$CANDIDATE_ARTIFACT")")/production_backup/spectral-bridge-server"
  mkdir -p "$(dirname "$PROMOTION_BACKUP")" "$(dirname "$BINARY")"
  [ -f "$BINARY" ] && cp -p "$BINARY" "$PROMOTION_BACKUP"
  CANDIDATE_TMP="$BINARY.self-change.tmp"
  cp -p "$CANDIDATE_ARTIFACT" "$CANDIDATE_TMP"
  [ "$(shasum -a 256 "$CANDIDATE_TMP" | awk '{print $1}')" = "$CANDIDATE_SHA256" ] \
    || fail_deploy "candidate artifact hash changed before install"
  mv "$CANDIDATE_TMP" "$BINARY"
  CANDIDATE_INSTALLED=true
  chmod 755 "$BINARY"
  BUILD_OK=true
  mkdir -p "$(dirname "$MANIFEST")"
  if ! python3 "$ASTRID/scripts/environment_receipts.py" manifest spectral-bridge \
    --output "$MANIFEST" \
    --repository "$ASTRID" \
    --artifact "spectral-bridge=$BINARY" \
    --artifact "self-change-promotion=$PROMOTION_HANDOFF" \
    --actor "$ACTOR" \
    --command "verified self-change promotion $PROMOTE_CANDIDATE" \
    >/dev/null; then
    fail_deploy "candidate promotion manifest could not be written"
  fi
elif [ "$DO_BUILD" -eq 1 ]; then
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

# 2b. Self-control lineage hand-off. Astrid's self-control V2 state is pinned
# to a deployment identity (commit + binary sha); startup refuses state from a
# stale deployment, so without a signed hand-off receipt EVERY redeploy
# silently voids her live self-regulation (BREATHE_ALONE/DRIFT/DAMPEN went
# "unwired" for two weeks after 2026-08-08 because no deploy prepared one).
# The receipt is one-shot, signed by the safety key, binds the exact state
# to THIS binary, and is consumed once at the next startup. "Already targets
# this deployment" is the normal no-op for restart-only runs.
SELF_CONTROL_HANDOFF="skipped"
if [ "$DO_RESTART" -eq 1 ] && [ -x "$BINARY" ]; then
  HANDOFF_OUT="$("$BINARY" --prepare-self-control-deployment-handoff \
    --operator-actor "$ACTOR" \
    --operator-ack "build_bridge.sh deploy (head=$HEAD): carry Astrid's self-control state to this build; lineage only, no control authority" 2>&1)" \
    && SELF_CONTROL_HANDOFF="prepared" \
    || { if printf '%s' "$HANDOFF_OUT" | grep -q "already targets this deployment"; then
           SELF_CONTROL_HANDOFF="already_current"
         else
           SELF_CONTROL_HANDOFF="failed"
           echo "build_bridge: WARNING self-control hand-off NOT prepared — her live self-regulation will be refused after restart until resolved:" >&2
           printf '%s\n' "$HANDOFF_OUT" | tail -3 >&2
         fi; }
fi
echo "build_bridge: self-control lineage hand-off: $SELF_CONTROL_HANDOFF"

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
if [ -n "$OLD_PID" ]; then
  OLD_STARTED_AT="$(ps -o lstart= -p "$OLD_PID" 2>/dev/null | awk '{$1=$1; print}')"
  OLD_COMMAND="$(ps -o command= -p "$OLD_PID" 2>/dev/null | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  OLD_CAPTURED_AT="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
fi
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

rm -f "$PROMOTION_VERIFY_JSON"
echo "build_bridge: done (head=$HEAD actor=$ACTOR built=$DO_BUILD restarted=1 pid=${OLD_PID:-none}->${NEW_PID:-?})"
