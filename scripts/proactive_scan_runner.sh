#!/bin/bash
# proactive_scan_runner.sh — the standing steward consumer (2026-08-18).
#
# Replaces the retired durable steward loop's signal-checking duty after the
# 2026-08 discovery that every being-feedback surface had rotted 25-46 days:
# steward_loop_run.sh was retired into an inert stub when steward_control.py
# shipped, and nothing was ever scheduled for steward duties again. This
# runner is deliberately DETERMINISTIC (no LLM): it runs the blind-spot
# probes, persists a durable report, and makes warnings impossible to miss
# (alert log + macOS notification + SessionStart surfacing). Action stays
# with interactive steward sessions; scan output is steward-only and must
# never reach being prompts.
#
# Also hosts the periodic steward-side data appends that the dead loop used
# to run (vibrancy/tail aperture durable history — steward-loop §5).
#
# launchd: com.astrid.proactive-scan (StartInterval 21600).
# Self-test: proactive_scan_runner.sh --self-test  (fabricated-warning path;
# the anti-drop catalog greps for this flag as the guard's test).

set -u

ASTRID="/Users/v/other/astrid"
OUT_DIR="$ASTRID/capsules/spectral-bridge/workspace/logs/proactive_scan"
ALERT_LOG="$OUT_DIR/steward_alerts.log"
LATEST="$OUT_DIR/latest.json"
KEEP_REPORTS=30
PY=/opt/homebrew/bin/python3

mkdir -p "$OUT_DIR"

summarize_warnings() {
    # stdin: blind-spots JSON; stdout: one line per warning finding
    "$PY" -c '
import json, sys
try:
    data = json.load(sys.stdin)
except Exception:
    print("scan_output_unparseable")
    sys.exit(0)
findings = data.get("findings", data) if isinstance(data, dict) else data
for f in findings or []:
    if isinstance(f, dict) and f.get("severity") == "warning":
        name = f.get("name", "?")
        summary = str(f.get("summary", ""))[:160]
        print(f"{name}: {summary}")
' 2>/dev/null
}

if [ "${1:-}" = "--self-test" ]; then
    # Fabricated-warning path: prove the alert plumbing without a real scan.
    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT
    echo '{"findings":[{"name":"selftest_probe","severity":"warning","summary":"fabricated warning for --self-test"}]}' > "$TMP/scan.json"
    LINES=$(summarize_warnings < "$TMP/scan.json")
    if [ -z "$LINES" ]; then
        echo "SELF-TEST FAIL: fabricated warning not summarized"; exit 1
    fi
    echo "SELF-TEST OK: $LINES"
    exit 0
fi

TS=$(date -u +%Y%m%dT%H%M%SZ)
REPORT="$OUT_DIR/scan_$TS.json"

# Steward-side durable data append the dead loop used to own (best-effort).
"$PY" "$ASTRID/scripts/watch_vibrancy_aperture.py" --append-history >/dev/null 2>&1 || true

# The scan itself (deterministic, ~30-60s).
if ! "$PY" "$ASTRID/scripts/proactive_scan.py" blind-spots --json > "$REPORT" 2>"$OUT_DIR/last_scan.err"; then
    echo "$(date -u +%FT%TZ) scan_failed rc=$? (see last_scan.err)" >> "$ALERT_LOG"
    exit 1
fi
cp "$REPORT" "$LATEST"

WARNINGS=$(summarize_warnings < "$REPORT")
if [ -n "$WARNINGS" ]; then
    {
        echo "$(date -u +%FT%TZ) warnings:"
        echo "$WARNINGS" | sed 's/^/  /'
    } >> "$ALERT_LOG"
    COUNT=$(echo "$WARNINGS" | wc -l | tr -d ' ')
    /usr/bin/osascript -e "display notification \"$COUNT blind-spot warning(s) — see steward_alerts.log\" with title \"Astrid steward scan\"" >/dev/null 2>&1 || true
fi

# Prune old reports (keep newest $KEEP_REPORTS).
ls -t "$OUT_DIR"/scan_*.json 2>/dev/null | tail -n +$((KEEP_REPORTS + 1)) | while read -r old; do
    rm -f "$old"
done

echo "$(date -u +%FT%TZ) scan ok: $(basename "$REPORT") warnings=$(echo "$WARNINGS" | grep -c . || true)"
exit 0
