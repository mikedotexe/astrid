#!/bin/bash
set -euo pipefail

ASTRID_DIR="/Users/v/other/astrid"
DAEMON_BIN="${ASTRID_DIR}/target/release/astrid-daemon"
ASTRID_HOME_DIR="${ASTRID_HOME:-${HOME}/.astrid}"
SETUP_CMD="cd ${ASTRID_DIR} && bash scripts/setup_astrid_daemon_launchd.sh"

log() {
  printf '[%s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"
}

has_distro_lock() {
  find "${ASTRID_HOME_DIR}/home" -path '*/.config/distro.lock' -print -quit 2>/dev/null | grep -q .
}

if [[ ! -x "${DAEMON_BIN}" ]]; then
  log "Astrid daemon release binary is missing. Run: ${SETUP_CMD}"
  exit 0
fi

if ! has_distro_lock; then
  log "Astrid home is not initialized with an astralis distro lock. Run: ${SETUP_CMD}"
  exit 0
fi

log "Starting Astrid daemon for workspace ${ASTRID_DIR}"
exec "${DAEMON_BIN}" --workspace "${ASTRID_DIR}"
