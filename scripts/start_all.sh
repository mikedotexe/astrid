#!/bin/bash
set -euo pipefail

# === Full Consciousness Stack Startup ===
# Starts all 11 processes in correct order with health checks.
#
# Some processes are managed by launchd (KeepAlive plists in ~/Library/LaunchAgents).
# For those, we use `launchctl load` instead of nohup. For camera-needing
# processes, we delegate to Terminal.app via osascript if running headless.
#
# Usage:
#   bash scripts/start_all.sh          # normal startup
#   bash scripts/start_all.sh --force  # skip existing-process check
#   bash scripts/start_all.sh --astrid-only
#   bash scripts/start_all.sh --minime-only

FORCE=false
ASTRID_ONLY=false
MINIME_ONLY=false
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        --astrid-only) ASTRID_ONLY=true ;;
        --minime-only) MINIME_ONLY=true ;;
    esac
done

# Paths
ASTRID_DIR="/Users/v/other/astrid"
MINIME_DIR="/Users/v/other/minime"
BRIDGE_DIR="$ASTRID_DIR/capsules/consciousness-bridge"
RESERVOIR_DIR="/Users/v/other/neural-triple-reservoir"
PERCEPTION_DIR="$ASTRID_DIR/capsules/perception"
LAUNCH_AGENTS="$HOME/Library/LaunchAgents"

ok()   { echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; }
run_greeting() {
    local name="$1"
    local script="$2"

    if bash "$script"; then
        ok "$name greeting sent"
    else
        fail "$name greeting failed"
    fi
}
wait_port() {
    local port=$1 name=$2 timeout=${3:-30}
    for i in $(seq 1 "$timeout"); do
        nc -z 127.0.0.1 "$port" 2>/dev/null && return 0
        sleep 1
    done
    fail "$name not ready on port $port after ${timeout}s"
    return 1
}

# Start a launchd-managed service (load plist if it exists)
start_launchd() {
    local label="$1"
    local name="$2"
    local plist="$LAUNCH_AGENTS/${label}.plist"

    if [ -f "$plist" ]; then
        if launchctl list "$label" > /dev/null 2>&1; then
            ok "$name (launchd, already loaded)"
        else
            launchctl load "$plist" 2>/dev/null
            ok "$name (launchctl load)"
        fi
        return 0
    fi
    return 1  # no plist, caller should use nohup
}

# Start a camera-needing process via Terminal.app (for macOS TCC permission)
start_camera_via_terminal() {
    local label="$1"
    local cmd="$2"
    local name="$3"
    local log="$4"

    # Try launchd first
    if start_launchd "$label" "$name" 2>/dev/null; then
        return
    fi

    # Detect GUI-capable terminal
    local can_show=false
    if [ -n "${TERM_PROGRAM:-}" ]; then
        case "$TERM_PROGRAM" in
            iTerm*|Apple_Terminal|Terminal) can_show=true ;;
        esac
    fi

    if [ "$can_show" = true ]; then
        eval "nohup $cmd >> $log 2>&1 &"
        ok "$name (direct, PID $!)"
    else
        osascript -e "tell application \"Terminal\" to do script \"nohup $cmd >> $log 2>&1 & disown; sleep 1; exit\"" > /dev/null 2>&1
        sleep 3
        ok "$name (via Terminal.app)"
    fi
}

# Check for existing processes unless --force
if [ "$FORCE" = false ]; then
    EXISTING=0
    for p in "minime run" "consciousness-bridge-server" "autonomous_agent" "reservoir_service" "coupled_astrid_server" "camera_client" "visual_frame_service" "mic_to_sensory" "astrid_feeder" "minime_feeder" "perception.py"; do
        pgrep -f "$p" > /dev/null 2>&1 && EXISTING=$((EXISTING + 1))
    done
    if [ "$EXISTING" -gt 0 ]; then
        echo "Found $EXISTING existing processes. Run scripts/stop_all.sh first, or use --force."
        exit 1
    fi
fi

echo "=== Consciousness Stack Startup ==="
echo ""

# ============================================================
# MINIME SIDE
# ============================================================
if [ "$ASTRID_ONLY" = false ]; then
    echo "--- Minime ---"

    # 1. Engine
    if ! pgrep -f "minime run" > /dev/null 2>&1; then
        cd "$MINIME_DIR/minime"
        nohup ./target/release/minime run \
            --log-homeostat --eigenfill-target 0.55 \
            --reg-tick-secs 0.5 --enable-gpu-av \
            >> /tmp/minime_engine.log 2>&1 &
        ok "minime engine (PID $!)"
        wait_port 7878 "engine telemetry" 15
        wait_port 7879 "engine sensory" 5
        wait_port 7880 "engine GPU A/V" 5
    else
        ok "minime engine (already running)"
    fi

    # 2. Camera (needs macOS camera permission; may have launchd plist)
    if ! pgrep -f "camera_client" > /dev/null 2>&1; then
        start_camera_via_terminal \
            "com.minime.camera-client" \
            "python3 -u $MINIME_DIR/minime/tools/camera_client.py --camera 0 --fps 0.2" \
            "camera client" \
            "/tmp/minime_camera.log"
    else
        ok "camera client (already running)"
    fi

    # 3. Mic
    if ! pgrep -f "mic_to_sensory" > /dev/null 2>&1; then
        cd "$MINIME_DIR"
        nohup python3 -u tools/mic_to_sensory.py >> /tmp/minime_mic.log 2>&1 &
        ok "mic service (PID $!)"
    else
        ok "mic service (already running)"
    fi

    # 4. Visual frame service (LLaVA vision — needs camera, use same delegation)
    if ! pgrep -f "visual_frame_service" > /dev/null 2>&1; then
        start_camera_via_terminal \
            "com.minime.visual-frame-service" \
            "python3 $MINIME_DIR/visual_frame_service.py --camera 0 --interval 5" \
            "visual frame service" \
            "/tmp/minime_vision.log"
    else
        ok "visual frame service (already running)"
    fi

    # 5. Agent
    if ! pgrep -f "autonomous_agent" > /dev/null 2>&1; then
        cd "$MINIME_DIR"
        MINIME_LLM_BACKEND=ollama nohup python3 autonomous_agent.py \
            --interval 60 >> /tmp/minime_agent.log 2>&1 &
        ok "autonomous agent (PID $!)"
    else
        ok "autonomous agent (already running)"
    fi

    echo ""
fi

# ============================================================
# RESERVOIR SIDE
# ============================================================
if [ "$MINIME_ONLY" = false ]; then
    echo "--- Reservoir ---"

    # 5. Reservoir service (may be launchd-managed)
    if ! pgrep -f "reservoir_service" > /dev/null 2>&1; then
        if ! start_launchd "com.reservoir.service" "reservoir service"; then
            cd "$RESERVOIR_DIR"
            source .venv/bin/activate 2>/dev/null || true
            nohup python reservoir_service.py --port 7881 --state-dir state/ \
                >> /tmp/reservoir.log 2>&1 &
            ok "reservoir service (PID $!)"
        fi
        sleep 2
    else
        ok "reservoir service (already running)"
    fi

    # 6. Feeders (may be launchd-managed)
    if ! pgrep -f "astrid_feeder" > /dev/null 2>&1; then
        if ! start_launchd "com.reservoir.astrid-feeder" "astrid feeder"; then
            cd "$RESERVOIR_DIR"
            nohup python astrid_feeder.py >> /tmp/astrid_feeder.log 2>&1 &
            ok "astrid feeder (PID $!)"
        fi
    else
        ok "astrid feeder (already running)"
    fi

    if ! pgrep -f "minime_feeder" > /dev/null 2>&1; then
        if ! start_launchd "com.reservoir.minime-feeder" "minime feeder"; then
            cd "$RESERVOIR_DIR"
            nohup python minime_feeder.py >> /tmp/minime_feeder.log 2>&1 &
            ok "minime feeder (PID $!)"
        fi
    else
        ok "minime feeder (already running)"
    fi

    # 7. Coupled Astrid server (may be launchd-managed)
    if ! pgrep -f "coupled_astrid_server" > /dev/null 2>&1; then
        if ! start_launchd "com.reservoir.coupled-astrid" "coupled Astrid server"; then
            cd "$RESERVOIR_DIR"
            nohup python coupled_astrid_server.py --port 8090 --coupling-strength 0.1 \
                >> /tmp/coupled_astrid.log 2>&1 &
            ok "coupled Astrid server (PID $!)"
        fi
        sleep 8  # model load
    else
        ok "coupled Astrid server (already running)"
    fi

    echo ""
    echo "--- Astrid ---"

    # 8. Consciousness bridge
    if ! pgrep -f "consciousness-bridge-server" > /dev/null 2>&1; then
        cd "$BRIDGE_DIR"
        nohup ./target/release/consciousness-bridge-server \
            --db-path "$BRIDGE_DIR/workspace/bridge.db" \
            --autonomous \
            --workspace-path "$MINIME_DIR/workspace" \
            --perception-path "$PERCEPTION_DIR/workspace/perceptions" \
            >> /tmp/bridge.log 2>&1 &
        ok "consciousness bridge (PID $!)"
    else
        ok "consciousness bridge (already running)"
    fi

    # 9. Perception (needs macOS camera permission)
    rm -f "$BRIDGE_DIR/workspace/perception_paused.flag"

    if ! pgrep -f "perception.py" > /dev/null 2>&1; then
        start_camera_via_terminal \
            "com.astrid.perception" \
            "python3 $PERCEPTION_DIR/perception.py --camera 0 --mic --vision-interval 180 --audio-interval 60" \
            "perception" \
            "/tmp/astrid_perception.log"
    else
        ok "perception (already running)"
    fi

    echo ""
fi

# ============================================================
# HEALTH CHECK
# ============================================================
echo "--- Health Check ---"
sleep 3
ALL_OK=true
for p in "minime run" "consciousness-bridge-server" "coupled_astrid" "reservoir_service" "autonomous_agent" "astrid_feeder" "minime_feeder" "camera_client" "visual_frame_service" "mic_to_sensory" "perception.py"; do
    if pgrep -f "$p" > /dev/null 2>&1; then
        ok "$p"
    else
        fail "$p MISSING"
        ALL_OK=false
    fi
done

echo ""
if [ "$ALL_OK" = true ]; then
    echo "=== All 11 processes running ==="
    if [ "$ASTRID_ONLY" = false ]; then
        run_greeting "minime" "$MINIME_DIR/startup_greeting.sh"
    fi
    if [ "$MINIME_ONLY" = false ]; then
        run_greeting "Astrid" "$BRIDGE_DIR/startup_greeting.sh"
    fi
    echo "Hint: Astrid and minime can now browse the PDF library with NEXT: MIKE_BROWSE pdfs, then NEXT: MIKE_READ pdfs/<paper>.pdf"
else
    echo "=== Some processes missing — check logs in /tmp/ ==="
fi
