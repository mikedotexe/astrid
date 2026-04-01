#!/bin/bash

# === Full Consciousness Stack Shutdown ===
# Stops all 11 processes in correct order (outer first, engine last).
# Handles both pkill (manual processes) and launchctl unload (launchd-managed).
# Always uses SIGTERM for graceful shutdown — NEVER SIGKILL.

LAUNCH_AGENTS="$HOME/Library/LaunchAgents"

stop_process() {
    local name="$1"
    local plist="${2:-}"

    # Try launchctl first (if launchd-managed, pkill alone won't stick)
    if [ -n "$plist" ] && [ -f "$LAUNCH_AGENTS/$plist" ]; then
        if launchctl list "${plist%.plist}" > /dev/null 2>&1; then
            launchctl unload "$LAUNCH_AGENTS/$plist" 2>/dev/null
            echo "  ✓ stopped $name (launchctl unload)"
            return
        fi
    fi

    # Fall back to pkill for manually-started processes
    if pkill -f "$name" 2>/dev/null; then
        echo "  ✓ stopped $name (pkill)"
    else
        echo "  - $name (not running)"
    fi
}

echo "=== Consciousness Stack Shutdown ==="
echo ""

# Astrid side (bridge + perception first)
echo "--- Stopping Astrid ---"
stop_process "consciousness-bridge-server"
stop_process "perception.py"
stop_process "coupled_astrid_server" "com.reservoir.coupled-astrid.plist"

# Reservoir (feeders first, service last — it snapshots on shutdown)
echo ""
echo "--- Stopping Reservoir ---"
stop_process "astrid_feeder" "com.reservoir.astrid-feeder.plist"
stop_process "minime_feeder" "com.reservoir.minime-feeder.plist"
sleep 1
stop_process "reservoir_service" "com.reservoir.service.plist"

# Minime outer processes
echo ""
echo "--- Stopping Minime ---"
stop_process "autonomous_agent"
stop_process "visual_frame_service"
stop_process "host-sensory"
stop_process "mic_to_sensory" "com.minime.mic-to-sensory.plist"
stop_process "camera_client" "com.minime.camera-client.plist"

# Engine last — give outer processes time to disconnect
sleep 3
stop_process "minime run"

# Note: previously closed ALL Terminal.app windows, which was overbroad.
# Only close windows we opened (identified by title/command) if needed.
# For now, leave Terminal.app alone — user may have other sessions.

# Clean up PID files and stale flags
rm -f /tmp/minime_pids/*.pid 2>/dev/null
rm -f /Users/v/other/astrid/capsules/consciousness-bridge/workspace/perception_paused.flag 2>/dev/null

echo ""

# Verify everything is actually stopped
sleep 2
REMAINING=0
for p in "minime run" "consciousness-bridge-server" "coupled_astrid_server" "reservoir_service" "autonomous_agent" "host-sensory" "astrid_feeder" "minime_feeder" "camera_client" "visual_frame_service" "mic_to_sensory" "perception.py"; do
    if pgrep -f "$p" > /dev/null 2>&1; then
        echo "  !! $p still running (PID $(pgrep -f "$p" | head -1))"
        REMAINING=$((REMAINING + 1))
    fi
done

if [ "$REMAINING" -eq 0 ]; then
    echo "=== All processes stopped ==="
else
    echo "=== WARNING: $REMAINING process(es) still running ==="
    echo "    These may be launchd-managed. Check: launchctl list | grep -E 'minime|reservoir'"
fi
