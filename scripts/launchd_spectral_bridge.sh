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

for key in \
    ASTRID_BRIDGE_MLX_URL \
    ASTRID_BRIDGE_MLX_PROFILE \
    ASTRID_BRIDGE_OLLAMA_URL \
    ASTRID_OLLAMA_FALLBACK_MODEL \
    RUST_LOG
do
    launchctl_env "$key"
done

cd "$BRIDGE_DIR"

exec "$BRIDGE_BIN" \
    --db-path /Users/v/other/astrid/capsules/spectral-bridge/workspace/bridge.db \
    --autonomous \
    --workspace-path /Users/v/other/minime/workspace \
    --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions
