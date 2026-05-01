#!/bin/bash
set -euo pipefail

# === Launchd-canonical consciousness stack startup ===
# Repo-owned plists are the source of truth. This script syncs those plists
# into ~/Library/LaunchAgents, bootstraps/kickstarts the expected labels, and
# reports drift. It intentionally avoids nohup/direct background starts.

FORCE=false
ASTRID_ONLY=false
MINIME_ONLY=false
SKIP_GREETING=false
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        --astrid-only) ASTRID_ONLY=true ;;
        --minime-only) MINIME_ONLY=true ;;
        --skip-greeting) SKIP_GREETING=true ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

ASTRID_DIR="/Users/v/other/astrid"
MINIME_DIR="/Users/v/other/minime"
BRIDGE_DIR="$ASTRID_DIR/capsules/consciousness-bridge"
RESERVOIR_DIR="/Users/v/other/neural-triple-reservoir"
LAUNCH_AGENTS="$HOME/Library/LaunchAgents"
DOMAIN="gui/$(id -u)"

ok() { echo "  OK $1"; }
warn() { echo "  !! $1"; }
fail() { echo "  XX $1"; }

label_for_plist() {
    basename "$1" .plist
}

installed_path_for() {
    printf '%s/%s\n' "$LAUNCH_AGENTS" "$(basename "$1")"
}

label_loaded() {
    local label="$1"
    launchctl print "$DOMAIN/$label" >/dev/null 2>&1
}

label_running() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | grep -q "state = running"
}

label_path() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | awk -F' = ' '/path = / {print $2; exit}'
}

sync_launch_agent() {
    local src="$1"
    local name
    name="$(basename "$src")"
    local dst="$LAUNCH_AGENTS/$name"
    SYNC_CHANGED=false

    if [ ! -f "$src" ]; then
        fail "$name source plist missing at $src"
        return 1
    fi

    mkdir -p "$LAUNCH_AGENTS"
    if [ ! -f "$dst" ] || ! cmp -s "$src" "$dst"; then
        cp "$src" "$dst"
        SYNC_CHANGED=true
        ok "$name synced to LaunchAgents"
    else
        ok "$name already synced"
    fi
}

bootout_label() {
    local label="$1"
    if label_loaded "$label"; then
        launchctl bootout "$DOMAIN/$label" >/dev/null 2>&1 || true
        for _ in $(seq 1 15); do
            if ! label_loaded "$label"; then
                return 0
            fi
            sleep 1
        done
        warn "$label still appears loaded after bootout; continuing with caution"
    fi
}

bootstrap_label() {
    local plist="$1"
    local label
    label="$(label_for_plist "$plist")"

    if launchctl bootstrap "$DOMAIN" "$plist" >/dev/null 2>&1; then
        ok "$label bootstrapped"
        return 0
    fi

    if label_loaded "$label"; then
        ok "$label already loaded"
        return 0
    fi

    fail "$label could not be bootstrapped from $plist"
    return 1
}

kickstart_label() {
    local label="$1"
    if launchctl kickstart -k "$DOMAIN/$label" >/dev/null 2>&1; then
        ok "$label kickstarted"
    else
        warn "$label kickstart unavailable"
    fi
}

ensure_launchd_label() {
    local src="$1"
    local description="$2"
    local mode="${3:-persistent}"
    local label
    label="$(label_for_plist "$src")"
    local installed
    installed="$(installed_path_for "$src")"

    sync_launch_agent "$src"

    if [ "$SYNC_CHANGED" = true ] && label_loaded "$label"; then
        warn "$label was loaded with stale plist; reloading from installed plist"
        bootout_label "$label"
    fi

    if label_loaded "$label"; then
        local loaded_path
        loaded_path="$(label_path "$label")"
        if [ -n "$loaded_path" ] && [ "$loaded_path" != "$installed" ]; then
            warn "$label is loaded from $loaded_path; reloading from installed plist"
            bootout_label "$label"
        fi
    fi

    if ! label_loaded "$label"; then
        bootstrap_label "$installed"
    fi

    if [ "$mode" = "oneshot" ]; then
        if [ "$SKIP_GREETING" = false ]; then
            kickstart_label "$label"
        else
            ok "$description installed; greeting run skipped by flag"
        fi
        return 0
    fi

    if label_running "$label"; then
        ok "$description running ($label)"
    else
        kickstart_label "$label"
    fi
}

wait_port() {
    local port="$1"
    local name="$2"
    local timeout="${3:-45}"
    for _ in $(seq 1 "$timeout"); do
        if nc -z 127.0.0.1 "$port" >/dev/null 2>&1 || \
           lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
            ok "$name ready on port $port"
            return 0
        fi
        sleep 1
    done
    fail "$name not ready on port $port after ${timeout}s"
    return 1
}

disable_opt_in_rescue_labels() {
    local loaded_any=false
    for label in com.minime.engine-rescue-watchdog com.minime.engine-rescue; do
        if label_loaded "$label"; then
            warn "$label is opt-in rescue mode; booting it out for normal launchd startup"
            bootout_label "$label"
            loaded_any=true
        fi
        if [ -f "$LAUNCH_AGENTS/$label.plist" ]; then
            warn "$label.plist is installed in LaunchAgents; cold boot may enter rescue mode"
        fi
    done
    if [ "$loaded_any" = true ]; then
        sleep 2
    fi
}

check_duplicate_processes() {
    if [ "$FORCE" = true ]; then
        return 0
    fi
    local duplicates=0
    for pattern in \
        "minime run" \
        "consciousness-bridge-server" \
        "reservoir_service" \
        "coupled_astrid_server" \
        "autonomous_agent" \
        "host-sensory" \
        "camera_client" \
        "visual_frame_service" \
        "mic_to_sensory" \
        "astrid_feeder" \
        "minime_feeder"
    do
        local count
        count="$(pgrep -f "$pattern" 2>/dev/null | awk 'NF' | wc -l | tr -d ' ')"
        if [ "$count" -gt 1 ]; then
            warn "$pattern has $count matching processes"
            duplicates=$((duplicates + 1))
        fi
    done
    if [ "$duplicates" -gt 0 ]; then
        echo "Duplicate processes detected. Run scripts/stop_all.sh first, or retry with --force after inspection." >&2
        exit 1
    fi
}

health_check_labels() {
    local all_ok=true
    for label in "$@"; do
        if label_running "$label"; then
            ok "$label"
        elif label_loaded "$label"; then
            warn "$label loaded but not running"
            all_ok=false
        else
            fail "$label missing"
            all_ok=false
        fi
    done

    if [ "$all_ok" = true ]; then
        return 0
    fi
    return 1
}

echo "=== Launchd Consciousness Stack Startup ==="
echo "Domain: $DOMAIN"
echo ""

check_duplicate_processes

EXPECTED_LABELS=()

if [ "$ASTRID_ONLY" = false ]; then
    echo "--- Minime ---"
    disable_opt_in_rescue_labels

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.engine.plist" "minime engine"
    wait_port 7878 "engine telemetry" 60
    wait_port 7879 "engine sensory" 15
    EXPECTED_LABELS+=("com.minime.engine")

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.host-sensory.plist" "host sensory"
    EXPECTED_LABELS+=("com.minime.host-sensory")

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.camera-client.plist" "camera client"
    EXPECTED_LABELS+=("com.minime.camera-client")

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.mic-to-sensory.plist" "mic service"
    EXPECTED_LABELS+=("com.minime.mic-to-sensory")

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.visual-frame-service.plist" "visual frame service"
    EXPECTED_LABELS+=("com.minime.visual-frame-service")

    ensure_launchd_label "$MINIME_DIR/launchd/com.minime.autonomous-agent.plist" "autonomous agent"
    EXPECTED_LABELS+=("com.minime.autonomous-agent")
    echo ""
fi

if [ "$MINIME_ONLY" = false ]; then
    echo "--- Reservoir ---"
    ensure_launchd_label "$RESERVOIR_DIR/launchd/com.reservoir.service.plist" "reservoir service"
    wait_port 7881 "reservoir service" 45
    EXPECTED_LABELS+=("com.reservoir.service")

    ensure_launchd_label "$RESERVOIR_DIR/launchd/com.reservoir.coupled-astrid.plist" "coupled Astrid server"
    wait_port 8090 "coupled Astrid server" 120
    EXPECTED_LABELS+=("com.reservoir.coupled-astrid")

    ensure_launchd_label "$RESERVOIR_DIR/launchd/com.reservoir.astrid-feeder.plist" "Astrid feeder"
    EXPECTED_LABELS+=("com.reservoir.astrid-feeder")

    ensure_launchd_label "$RESERVOIR_DIR/launchd/com.reservoir.minime-feeder.plist" "Minime feeder"
    EXPECTED_LABELS+=("com.reservoir.minime-feeder")
    echo ""

    echo "--- Astrid ---"
    ensure_launchd_label "$ASTRID_DIR/launchd/com.astrid.consciousness-bridge.plist" "consciousness bridge"
    EXPECTED_LABELS+=("com.astrid.consciousness-bridge")

    ensure_launchd_label "$ASTRID_DIR/launchd/com.astrid.calm-startup-greeting.plist" "calm startup greeting" "oneshot"
    echo ""
fi

echo "--- Health Check ---"
if health_check_labels "${EXPECTED_LABELS[@]}"; then
    echo "=== All expected launchd labels are running ==="
else
    echo "=== Some launchd labels are missing or not running ==="
    exit 1
fi

echo ""
echo "--- Launchd Inventory ---"
if [ -x "$ASTRID_DIR/scripts/launchd_inventory.sh" ]; then
    "$ASTRID_DIR/scripts/launchd_inventory.sh" || true
else
    warn "launchd inventory helper missing"
fi
