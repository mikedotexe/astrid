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
    "$RESERVOIR_DIR/launchd/com.reservoir.service.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.coupled-astrid.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.astrid-feeder.plist"
    "$RESERVOIR_DIR/launchd/com.reservoir.minime-feeder.plist"
    "$ASTRID_DIR/launchd/com.astrid.consciousness-bridge.plist"
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
    com.reservoir.service
    com.reservoir.coupled-astrid
    com.reservoir.astrid-feeder
    com.reservoir.minime-feeder
    com.astrid.consciousness-bridge
    com.astrid.perception-host-ascii
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
