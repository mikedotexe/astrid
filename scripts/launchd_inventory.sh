#!/bin/bash
set -euo pipefail

STRICT=false
for arg in "$@"; do
    case "$arg" in
        --strict) STRICT=true ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

ASTRID_DIR="/Users/v/other/astrid"
MINIME_DIR="/Users/v/other/minime"
RESERVOIR_DIR="/Users/v/other/neural-triple-reservoir"
LAUNCH_AGENTS="$HOME/Library/LaunchAgents"
DOMAIN="gui/$(id -u)"
FAILURES=0
WARNINGS=0

prod_plists=(
    "$MINIME_DIR/launchd/com.minime.engine.plist"
    "$MINIME_DIR/launchd/com.minime.host-sensory.plist"
    "$MINIME_DIR/launchd/com.minime.camera-client.plist"
    "$MINIME_DIR/launchd/com.minime.mic-to-sensory.plist"
    "$MINIME_DIR/launchd/com.minime.visual-frame-service.plist"
    "$MINIME_DIR/launchd/com.minime.autonomous-agent.plist"
    "$MINIME_DIR/launchd/com.minime.usb-hotplug-watchdog.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.service.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.coupled-astrid.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.astrid-feeder.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.minime-feeder.plist"
    "$ASTRID_DIR/launchd/com.astrid.daemon.plist"
    "$ASTRID_DIR/launchd/com.astrid.spectral-bridge.plist"
    "$ASTRID_DIR/launchd/com.astrid.perception-host-ascii.plist"
    "$ASTRID_DIR/launchd/com.astrid.calm-startup-greeting.plist"
)

persistent_labels=(
    com.minime.engine
    com.minime.host-sensory
    com.minime.camera-client
    com.minime.mic-to-sensory
    com.minime.visual-frame-service
    com.minime.autonomous-agent
    com.minime.usb-hotplug-watchdog
    com.reservoir.service
    com.reservoir.coupled-astrid
    com.reservoir.astrid-feeder
    com.reservoir.minime-feeder
    com.astrid.daemon
    com.astrid.spectral-bridge
    com.astrid.perception-host-ascii
)

legacy_labels=(
    com.astrid.consciousness-bridge
)

opt_in_plists=(
    "$MINIME_DIR/launchd/com.minime.engine-rescue.plist"
    "$MINIME_DIR/launchd/com.minime.engine-rescue-watchdog.plist"
)

ok() { echo "  OK $1"; }
warn() { echo "  !! $1"; WARNINGS=$((WARNINGS + 1)); }
fail() { echo "  XX $1"; FAILURES=$((FAILURES + 1)); }

label_for_plist() {
    basename "$1" .plist
}

plist_value() {
    local plist="$1"
    local dotted_key="$2"
    python3 - "$plist" "$dotted_key" <<'PY' 2>/dev/null || true
import plistlib
import sys

with open(sys.argv[1], "rb") as f:
    value = plistlib.load(f)
for key in sys.argv[2].split("."):
    if not isinstance(value, dict):
        value = None
        break
    value = value.get(key)
if value is None:
    print("")
else:
    print(value)
PY
}

label_loaded() {
    local label="$1"
    launchctl print "$DOMAIN/$label" >/dev/null 2>&1
}

label_state() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/state = / {print $2; exit}'
}

label_pid() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/pid = / {print $2; exit}'
}

label_path() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/path = / {print $2; exit}'
}

echo "=== launchd inventory ==="
echo "Domain: $DOMAIN"
echo ""

echo "--- Repo to ~/Library/LaunchAgents parity ---"
for src in "${prod_plists[@]}"; do
    label="$(label_for_plist "$src")"
    installed="$LAUNCH_AGENTS/$(basename "$src")"
    if [ ! -f "$src" ]; then
        fail "$label source missing: $src"
    elif [ ! -f "$installed" ]; then
        fail "$label not installed at $installed"
    elif cmp -s "$src" "$installed"; then
        ok "$label installed and matches repo"
    else
        fail "$label installed plist differs from repo"
    fi
done

echo ""
echo "--- Loaded labels ---"
for label in "${persistent_labels[@]}"; do
    if label_loaded "$label"; then
        state="$(label_state "$label")"
        pid="$(label_pid "$label")"
        loaded_path="$(label_path "$label")"
        installed="$LAUNCH_AGENTS/$label.plist"
        if [ "$state" = "running" ] && [ "$loaded_path" = "$installed" ]; then
            ok "$label running pid=${pid:-?} path=${loaded_path:-?}"
        elif [ "$state" = "running" ]; then
            fail "$label running from ${loaded_path:-?}, expected $installed"
        else
            fail "$label loaded but state=${state:-unknown}"
        fi
    else
        fail "$label not loaded"
    fi
done

if label_loaded com.astrid.calm-startup-greeting; then
    state="$(label_state com.astrid.calm-startup-greeting)"
    ok "com.astrid.calm-startup-greeting loaded state=${state:-unknown} (one-shot)"
else
    warn "com.astrid.calm-startup-greeting not loaded in this session; it will run at next login if installed"
fi

echo ""
echo "--- Opt-in rescue labels ---"
for src in "${opt_in_plists[@]}"; do
    label="$(label_for_plist "$src")"
    installed="$LAUNCH_AGENTS/$(basename "$src")"
    if [ -f "$installed" ]; then
        fail "$label is installed in LaunchAgents; rescue should be opt-in, not cold-boot default"
    elif label_loaded "$label"; then
        state="$(label_state "$label")"
        pid="$(label_pid "$label")"
        warn "$label loaded for current session only; state=${state:-unknown} pid=${pid:-?}"
    else
        ok "$label not installed and not loaded"
    fi
done

echo ""
echo "--- Legacy renamed labels ---"
for label in "${legacy_labels[@]}"; do
    installed="$LAUNCH_AGENTS/$label.plist"
    if [ -f "$installed" ]; then
        fail "$label legacy plist still installed at $installed"
    elif label_loaded "$label"; then
        state="$(label_state "$label")"
        pid="$(label_pid "$label")"
        fail "$label legacy label still loaded; state=${state:-unknown} pid=${pid:-?}"
    else
        ok "$label not installed and not loaded"
    fi
done

echo ""
echo "--- Minime normal engine environment ---"
engine_plist="$LAUNCH_AGENTS/com.minime.engine.plist"
if [ -f "$engine_plist" ]; then
    target="$(plist_value "$engine_plist" "EnvironmentVariables.EIGENFILL_TARGET")"
    hard_reset="$(plist_value "$engine_plist" "EnvironmentVariables.MINIME_HARD_RECOVERY_RESET")"
    legacy_audio="$(plist_value "$engine_plist" "EnvironmentVariables.LEGACY_AUDIO_ENABLED")"
    legacy_video="$(plist_value "$engine_plist" "EnvironmentVariables.LEGACY_VIDEO_ENABLED")"
    if [ "$target" = "0.68" ]; then
        ok "com.minime.engine EIGENFILL_TARGET=$target"
    else
        fail "com.minime.engine EIGENFILL_TARGET=$target (expected 0.68)"
    fi
    if [ -z "$hard_reset" ]; then
        ok "MINIME_HARD_RECOVERY_RESET is derived by launch wrapper, not pinned in plist"
    else
        fail "MINIME_HARD_RECOVERY_RESET is pinned in installed plist ($hard_reset)"
    fi
    ok "legacy audio/video flags: audio=${legacy_audio:-unset}, video=${legacy_video:-unset}"
else
    fail "com.minime.engine plist missing"
fi

session_target="$(launchctl getenv EIGENFILL_TARGET 2>/dev/null || true)"
if [ "$session_target" = "0.68" ] || [ -z "$session_target" ]; then
    ok "launchctl session EIGENFILL_TARGET=${session_target:-unset}"
else
    fail "launchctl session EIGENFILL_TARGET=$session_target (expected unset or 0.68)"
fi

echo ""
echo "--- Astrid daemon environment ---"
astrid_daemon_plist="$LAUNCH_AGENTS/com.astrid.daemon.plist"
astrid_daemon_bin="$ASTRID_DIR/target/release/astrid-daemon"
astrid_daemon_socket="$HOME/.astrid/run/system.sock"
if [ -f "$astrid_daemon_plist" ]; then
    workdir="$(plist_value "$astrid_daemon_plist" "WorkingDirectory")"
    if [ "$workdir" = "$ASTRID_DIR" ]; then
        ok "com.astrid.daemon WorkingDirectory=$workdir"
    else
        fail "com.astrid.daemon WorkingDirectory=${workdir:-unset} (expected $ASTRID_DIR)"
    fi
else
    fail "com.astrid.daemon plist missing"
fi
if [ -x "$ASTRID_DIR/scripts/launchd_astrid_daemon.sh" ]; then
    ok "Astrid daemon launch wrapper executable"
else
    fail "Astrid daemon launch wrapper missing or not executable"
fi
if [ -x "$astrid_daemon_bin" ]; then
    ok "Astrid daemon release binary present"
else
    fail "Astrid daemon release binary missing at $astrid_daemon_bin"
fi
if [ -S "$astrid_daemon_socket" ]; then
    ok "Astrid daemon socket present at $astrid_daemon_socket"
else
    fail "Astrid daemon socket missing at $astrid_daemon_socket"
fi
capsule_health_json="$(python3 "$ASTRID_DIR/scripts/capsule_runtime_health.py" --json 2>/dev/null || true)"
if [ -z "$capsule_health_json" ]; then
    fail "Capsule runtime health probe failed"
else
    capsule_health_summary="$(printf '%s' "$capsule_health_json" | python3 -c 'import json,sys; s=json.load(sys.stdin).get("summary", {}); text="{installed} installed, {discovered} discovered, {component} Component Model, {accepted}/{legacy} accepted legacy, {incompatible} incompatible, {missing} missing".format(installed=s.get("installed_manifests", 0), discovered=s.get("discovered_manifests", 0), component=s.get("loadable_component_model", 0), accepted=s.get("accepted_legacy_extism_mvp", 0), legacy=s.get("legacy_extism_mvp", 0), incompatible=s.get("actionable_incompatible", 0), missing=s.get("actionable_missing_payloads", 0)); print("{}|{}".format(s.get("status", "unknown"), text))' 2>/dev/null || true)"
    if [ -z "$capsule_health_summary" ]; then
        fail "Capsule runtime health JSON could not be parsed"
    else
        IFS='|' read -r capsule_health_status capsule_health_text <<< "$capsule_health_summary"
        if [ "$capsule_health_status" = "ok" ]; then
            ok "Capsule runtime health $capsule_health_status: $capsule_health_text"
        else
            fail "Capsule runtime health $capsule_health_status: $capsule_health_text"
        fi
    fi
fi

echo ""
echo "--- Astrid perception environment ---"
astrid_perception_plist="$LAUNCH_AGENTS/com.astrid.perception-host-ascii.plist"
if [ -f "$astrid_perception_plist" ]; then
    look_source="$(plist_value "$astrid_perception_plist" "EnvironmentVariables.LOOK_SOURCE")"
    camera_index="$(plist_value "$astrid_perception_plist" "EnvironmentVariables.ASTRID_CAMERA_INDEX")"
    enable_mic="$(plist_value "$astrid_perception_plist" "EnvironmentVariables.ASTRID_ENABLE_MIC")"
    if [ "$look_source" = "active" ]; then
        ok "Astrid LOOK_SOURCE=$look_source"
    else
        fail "Astrid LOOK_SOURCE=${look_source:-unset} (expected active)"
    fi
    if [ "$camera_index" = "0" ]; then
        ok "Astrid camera index=$camera_index"
    else
        fail "Astrid camera index=${camera_index:-unset} (expected 0)"
    fi
    if [ "$enable_mic" = "1" ]; then
        ok "Astrid mic enabled by default"
    else
        fail "Astrid mic flag=${enable_mic:-unset} (expected 1)"
    fi
else
    fail "com.astrid.perception-host-ascii plist missing"
fi

echo ""
echo "Summary: failures=$FAILURES warnings=$WARNINGS"
if [ "$STRICT" = true ] && [ "$FAILURES" -gt 0 ]; then
    exit 1
fi
