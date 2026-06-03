#!/bin/bash
set -euo pipefail

ASTRID_DIR="/Users/v/other/astrid"
ASTRID_BIN="${ASTRID_DIR}/target/release/astrid"
PLIST_SOURCE="${ASTRID_DIR}/launchd/com.astrid.daemon.plist"
PLIST_DEST="${HOME}/Library/LaunchAgents/com.astrid.daemon.plist"
LABEL="com.astrid.daemon"
DOMAIN="gui/$(id -u)"
SOCKET="${HOME}/.astrid/run/system.sock"

log() {
  printf '[%s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"
}

log "Building bundled Astrid release binaries"
(cd "${ASTRID_DIR}" && cargo build -p astrid --release --bins)

log "Initializing Astrid home with astralis distro"
"${ASTRID_BIN}" init --distro astralis

log "Installing ${LABEL} launch agent"
mkdir -p "${HOME}/Library/LaunchAgents"
cp "${PLIST_SOURCE}" "${PLIST_DEST}"
chmod 644 "${PLIST_DEST}"

if launchctl print "${DOMAIN}/${LABEL}" >/dev/null 2>&1; then
  log "Reloading existing ${LABEL}"
  launchctl bootout "${DOMAIN}" "${PLIST_DEST}" >/dev/null 2>&1 || true
fi

launchctl bootstrap "${DOMAIN}" "${PLIST_DEST}"
launchctl kickstart -k "${DOMAIN}/${LABEL}"

log "Waiting for Astrid daemon socket"
for _ in $(seq 1 60); do
  if [[ -S "${SOCKET}" ]]; then
    break
  fi
  sleep 1
done

if [[ ! -S "${SOCKET}" ]]; then
  log "Astrid daemon socket did not appear at ${SOCKET}"
  exit 1
fi

log "Verifying Astrid daemon status"
"${ASTRID_BIN}" --format json status
