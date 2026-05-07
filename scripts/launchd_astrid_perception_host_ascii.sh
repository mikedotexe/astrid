#!/bin/bash
set -euo pipefail

ASTRID_DIR="/Users/v/other/astrid"
PERCEPTION_DIR="$ASTRID_DIR/capsules/perception"
PYTHON_BIN="${PYTHON_BIN:-python3}"
LOOK_SOURCE="${LOOK_SOURCE:-host}"
ASCII_INTERVAL="${ASTRID_HOST_ASCII_INTERVAL:-45}"
MINIME_WORKSPACE="${MINIME_WORKSPACE:-/Users/v/other/minime/workspace}"
export MINIME_WORKSPACE

cd "$PERCEPTION_DIR"

exec "$PYTHON_BIN" "$PERCEPTION_DIR/perception.py" \
    --ascii-source "$LOOK_SOURCE" \
    --ascii-interval "$ASCII_INTERVAL" \
    --vision-interval 999999 \
    --audio-interval 999999
