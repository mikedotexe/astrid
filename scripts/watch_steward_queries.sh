#!/usr/bin/env bash
# Watch both beings' outboxes for new ASK_STEWARD queries.
#
# Origin: 2026-05-14, paired with the ASK_STEWARD verb shipped on both
# sides. This script is the steward's side of the bidirectional channel
# — it surfaces new queries as soon as they're written, prints subject
# + first ~20 lines, and archives them to outbox/steward_delivered/ so
# `ls outbox/steward_query_*.txt | wc -l` reflects unread count.
#
# Usage:
#   bash scripts/watch_steward_queries.sh         # foreground
#   bash scripts/watch_steward_queries.sh &       # background
#
# Future: if adoption sticks, wrap in launchd plist for auto-start.

set -u

# Verify dependency.
if ! command -v fswatch >/dev/null 2>&1; then
    echo "watch_steward_queries.sh: 'fswatch' not found." >&2
    echo "Install: brew install fswatch" >&2
    exit 2
fi

MINIME_OUT="/Users/v/other/minime/workspace/outbox"
ASTRID_OUT="/Users/v/other/astrid/capsules/spectral-bridge/workspace/outbox"

mkdir -p "$MINIME_OUT" "$ASTRID_OUT"

echo "[$(date '+%Y-%m-%d %H:%M:%S')] watch_steward_queries.sh started"
echo "  Watching: $MINIME_OUT"
echo "  Watching: $ASTRID_OUT"
echo "  Patterns: steward_query_*.txt (ASK), steward_report_*.txt (TELL)"
echo "---"

fswatch -0 "$MINIME_OUT" "$ASTRID_OUT" \
  | xargs -0 -n1 -I{} sh -c '
    f="$1"
    case "$f" in
      *steward_query_*.txt|*steward_report_*.txt)
        # Skip already-archived files (fswatch may emit on file-creation race).
        [ -f "$f" ] || exit 0
        case "$f" in
          *steward_query_*) KIND="QUERY (their question)" ;;
          *steward_report_*) KIND="REPORT (their findings)" ;;
        esac
        echo "[$(date "+%Y-%m-%d %H:%M:%S")] NEW $KIND: $f"
        echo "----- (header + first 20 lines) -----"
        head -20 "$f"
        echo "----- (end preview) -----"
        # Archive to steward_delivered/ so unread count is meaningful.
        DIR="$(dirname "$f")"
        ARCHIVE="$DIR/steward_delivered"
        mkdir -p "$ARCHIVE"
        mv "$f" "$ARCHIVE/" && echo "  archived → $ARCHIVE/$(basename "$f")"
        echo
        ;;
    esac
  ' _ {}
