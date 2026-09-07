#!/usr/bin/env bash
# Supported wrapper for isolated Astrid/Minime self-change shadow processes.
set -euo pipefail

ASTRID="/Users/v/other/astrid"
COMMAND="${1:-}"
case "$COMMAND" in
  start|observe|promote|rollback|verify-promotion) ;;
  *)
    echo "usage: run_self_change_canary.sh <start|observe|promote|rollback|verify-promotion> [args...]" >&2
    exit 64
    ;;
esac
shift
export ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER=1
exec python3 "$ASTRID/scripts/self_change_canary.py" "$COMMAND" "$@"
