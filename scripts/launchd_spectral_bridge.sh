#!/bin/bash
set -euo pipefail

BRIDGE_BIN="/Users/v/other/astrid/capsules/spectral-bridge/target/release/spectral-bridge-server"
BRIDGE_DIR="/Users/v/other/astrid/capsules/spectral-bridge"

launchctl_env() {
    local key="$1"
    local value
    value="$(/bin/launchctl getenv "$key" 2>/dev/null || true)"
    if [ -n "$value" ]; then
        export "$key=$value"
    fi
}

# Durable aperture-ceiling decisions (consent-sensitive shared-substrate dials that must survive
# reboot — a bare `launchctl setenv` is wiped on reboot, which would silently re-muffle the dials).
# The config file is the decision record; source it so the bridge inherits the ceilings even after
# a reboot. A live `launchctl setenv` override (imported by the loop below) still wins for tuning.
APERTURE_CONFIG="/Users/v/other/astrid/capsules/spectral-bridge/workspace/runtime/aperture_ceilings.env"
if [ -f "$APERTURE_CONFIG" ]; then
    # shellcheck source=/dev/null
    . "$APERTURE_CONFIG"
fi

for key in \
    ASTRID_BRIDGE_MLX_URL \
    ASTRID_BRIDGE_MLX_PROFILE \
    ASTRID_BRIDGE_OLLAMA_URL \
    ASTRID_OLLAMA_FALLBACK_MODEL \
    ASTRID_VIBRANCY_APERTURE_CEILING \
    ASTRID_TAIL_PARTICIPATION_CEILING \
    ASTRID_PRESSURE_ATTENUATION \
    RUST_LOG
do
    launchctl_env "$key"
done

# Re-publish the effective aperture ceilings to the launchd domain so external read-only monitors
# (e.g. scripts/watch_vibrancy_aperture.py) report the same value the bridge is actually using —
# otherwise, post-reboot, the domain would read empty while the bridge runs the sourced config value.
[ -n "${ASTRID_VIBRANCY_APERTURE_CEILING:-}" ] && /bin/launchctl setenv ASTRID_VIBRANCY_APERTURE_CEILING "$ASTRID_VIBRANCY_APERTURE_CEILING" 2>/dev/null || true
[ -n "${ASTRID_TAIL_PARTICIPATION_CEILING:-}" ] && /bin/launchctl setenv ASTRID_TAIL_PARTICIPATION_CEILING "$ASTRID_TAIL_PARTICIPATION_CEILING" 2>/dev/null || true
[ -n "${ASTRID_PRESSURE_ATTENUATION:-}" ] && /bin/launchctl setenv ASTRID_PRESSURE_ATTENUATION "$ASTRID_PRESSURE_ATTENUATION" 2>/dev/null || true

cd "$BRIDGE_DIR"

exec "$BRIDGE_BIN" \
    --db-path /Users/v/other/astrid/capsules/spectral-bridge/workspace/bridge.db \
    --autonomous \
    --workspace-path /Users/v/other/minime/workspace \
    --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions
