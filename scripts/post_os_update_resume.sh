#!/bin/bash
# post_os_update_resume.sh — one-command resume after an OS update/restart
# (written 2026-08-18 for the maintenance-restart runbook).
#
# Pre-shutdown state this pairs with:
#   - steward controller PAUSED (actor mike-os-update; persists on disk)
#   - viscosity candidate self-change-1bda788b60e743dbbf22daaa rolled back
#     (shadow stopped; restage is a steward action, printed at the end)
#   - division-child labels DISABLED at the launchd level (persists;
#     `launchctl enable gui/501/<label>` when fresh division intents exist)
#   - aux timer jobs booted out (they auto-reload from ~/Library/LaunchAgents
#     at login: flywheel, shadow-sample-recorder, research-budget-approver,
#     test-proposal-applier, proactive-scan)
#
# What this script does: verify the stack came back healthy, then RESUME the
# steward controller so the flywheel's next fire proceeds. It never starts
# or restarts stack processes itself — login/launchd owns that; if health
# fails, it says so and stops before resuming.

set -u
ASTRID="/Users/v/other/astrid"
MINIME="/Users/v/other/minime"
PASS=0; FAIL=0
ok()   { echo "  OK $1"; PASS=$((PASS+1)); }
bad()  { echo "  XX $1"; FAIL=$((FAIL+1)); }

echo "=== post-OS-update resume ($(date)) ==="

echo "--- process stack ---"
for p in "minime run" "spectral-bridge-server" "coupled_astrid_server" \
         "reservoir_service" "autonomous_agent" "astrid_feeder" "minime_feeder" \
         "camera_client" "mic_to_sensory" "perception.py"; do
    pgrep -f "$p" > /dev/null && ok "$p" || bad "$p MISSING"
done

echo "--- zombie / liveness ---"
MIC_RMS=$(tail -2 "$MINIME/logs/mic-to-sensory.log" 2>/dev/null | grep -oE "RMS[= ]0\.[0-9]+" | tail -1)
case "$MIC_RMS" in
    *"0.000"*) bad "mic RMS=0.000 (zombie — launchctl unload/load com.minime.mic-to-sensory)" ;;
    "") bad "mic log unreadable" ;;
    *) ok "mic $MIC_RMS" ;;
esac
tail -2 "$MINIME/logs/camera-client.log" 2>/dev/null | grep -q "Sent" && ok "camera sending frames" || bad "camera not sending (check TCC / usb watchdog)"
curl -s --max-time 5 http://127.0.0.1:8090/v1/models | grep -q "gemma" && ok "MLX 8090 serving" || bad "MLX 8090 not answering (model may still be loading — retry in 2 min)"

echo "--- sensory sources ---"
python3 "$MINIME/scripts/sensory_source_check.py" 2>/dev/null | head -6 || bad "sensory_source_check failed"

echo "--- launchd inventory ---"
if bash "$ASTRID/scripts/launchd_inventory.sh" --strict >/dev/null 2>&1; then
    ok "inventory strict clean"
else
    bad "inventory drift — run: bash scripts/launchd_inventory.sh --strict"
fi
for l in com.minime.division-child-minime com.minime.division-child-astrid; do
    launchctl print "gui/$(id -u)/$l" >/dev/null 2>&1 && bad "$l LOADED (must stay unloaded)" || ok "$l unloaded (disabled)"
done
for l in com.astrid.introspection-flywheel com.astrid.proactive-scan \
         com.astrid.shadow-sample-recorder com.astrid.research-budget-approver; do
    launchctl print "gui/$(id -u)/$l" >/dev/null 2>&1 && ok "$l loaded" || bad "$l NOT loaded — launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/$l.plist"
done

echo ""
if [ "$FAIL" -gt 0 ]; then
    echo "RESULT: $FAIL check(s) failed — NOT resuming the steward controller."
    echo "Fix the failures (or wait for slow starters like MLX), re-run this script."
    exit 1
fi

echo "--- all green: resuming steward controller ---"
python3 "$ASTRID/scripts/steward_control.py" resume --actor post-os-update-resume 2>&1 | tail -2
echo ""
echo "RESULT: healthy + resumed. The flywheel's next launchd fire proceeds normally."
echo ""
echo "Remaining steward action (manual, when ready):"
echo "  Restage Astrid's viscosity candidate (rolled back for the restart, no"
echo "  consent line had been written). Candidate ids are content-derived, so"
echo "  clear the rolled-back dirs first:"
echo "    rm -rf ~/.astrid/self_change_candidates/astrid/self-change-1bda788b60e743dbbf22daaa \\"
echo "           /Users/v/other/.self_change_self-change-1bda788b60e743dbbf22daaa"
echo "    python3 scripts/self_change_pipeline.py stage --request \\"
echo "      capsules/spectral-bridge/workspace/diagnostics/self_change_pipeline_v1/codraft_agency_code_change_1784594224.json \\"
echo "      --write --canary-secs 1800 --confirmation-grace-secs 14400"
echo "  then soak + invite as before (the fresh invite should briefly note the"
echo "  first window was cut short by maintenance, not by her)."
exit 0
