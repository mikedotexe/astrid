#!/bin/bash
set -euo pipefail

# Sends the short startup orientation notes once per boot. This script is safe
# to call from launchd and from scripts/start_all.sh; it refuses duplicates
# unless --force is supplied.

FORCE=false
ASTRID_ONLY=false
MINIME_ONLY=false
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        --astrid-only) ASTRID_ONLY=true ;;
        --minime-only) MINIME_ONLY=true ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

ASTRID_DIR="/Users/v/other/astrid"
MINIME_DIR="/Users/v/other/minime"
BRIDGE_DIR="$ASTRID_DIR/capsules/consciousness-bridge"
STAMP_DIR="$BRIDGE_DIR/workspace/runtime"
STAMP_FILE="$STAMP_DIR/startup_greeting_boot_id"
DOMAIN="gui/$(id -u)"

boot_id="$(sysctl -n kern.boottime 2>/dev/null | sed -E 's/^\{ sec = ([0-9]+),.*/\1/' || true)"
if [ -z "$boot_id" ]; then
    boot_id="$(date +%Y-%m-%d)"
fi

mkdir -p "$STAMP_DIR"
if [ "$FORCE" = false ] && [ -f "$STAMP_FILE" ] && [ "$(cat "$STAMP_FILE" 2>/dev/null)" = "$boot_id" ]; then
    echo "Calm startup greeting already sent for boot $boot_id"
    exit 0
fi

label_running() {
    local label="$1"
    launchctl print "$DOMAIN/$label" 2>/dev/null | grep -q "state = running"
}

wait_for_label() {
    local label="$1"
    local timeout="${2:-180}"
    for _ in $(seq 1 "$timeout"); do
        if label_running "$label"; then
            return 0
        fi
        sleep 1
    done
    return 1
}

if [ "$ASTRID_ONLY" = false ]; then
    wait_for_label com.minime.engine 180 || echo "Minime engine not ready before greeting timeout" >&2
    wait_for_label com.minime.autonomous-agent 120 || echo "Minime autonomous agent not ready before greeting timeout" >&2
fi

if [ "$MINIME_ONLY" = false ]; then
    wait_for_label com.astrid.consciousness-bridge 180 || echo "Astrid bridge not ready before greeting timeout" >&2
fi

if [ "$ASTRID_ONLY" = false ]; then
    bash "$MINIME_DIR/startup_greeting.sh"
fi

if [ "$MINIME_ONLY" = false ]; then
    bash "$BRIDGE_DIR/startup_greeting.sh"
fi

printf '%s\n' "$boot_id" > "$STAMP_FILE"
echo "Calm startup greeting sent for boot $boot_id"
