#!/bin/bash
set -euo pipefail

ASTRID_DIR="/Users/v/other/astrid"
PERCEPTION_DIR="$ASTRID_DIR/capsules/perception"
PYTHON_BIN="${PYTHON_BIN:-python3}"
LOOK_SOURCE="${LOOK_SOURCE:-active}"
ASCII_INTERVAL="${ASTRID_HOST_ASCII_INTERVAL:-45}"
VISION_INTERVAL="${ASTRID_VISION_INTERVAL:-60}"
AUDIO_INTERVAL="${ASTRID_AUDIO_INTERVAL:-30}"
CAMERA_INDEX="${ASTRID_CAMERA_INDEX:-0}"
ENABLE_MIC="${ASTRID_ENABLE_MIC:-1}"
MINIME_WORKSPACE="${MINIME_WORKSPACE:-/Users/v/other/minime/workspace}"
export MINIME_WORKSPACE

cd "$PERCEPTION_DIR"

args=(
    "$PERCEPTION_DIR/perception.py"
    --camera "$CAMERA_INDEX"
    --ascii-source "$LOOK_SOURCE"
    --ascii-interval "$ASCII_INTERVAL"
    --vision-interval "$VISION_INTERVAL"
    --audio-interval "$AUDIO_INTERVAL"
)

if [ "$ENABLE_MIC" != "0" ]; then
    args+=(--mic)
fi

exec "$PYTHON_BIN" "${args[@]}"
