#!/usr/bin/env bash
# Diagnose launchctl env-var propagation for the consciousness-bridge stack.
#
# Today's discovery (Kink #6, 2026-05-14):
#   `launchctl setenv FOO bar` does NOT propagate to launchd-managed
#   processes whose plist defines an `EnvironmentVariables` block.
#   The plist EV block REPLACES the inherited launchd-domain env;
#   `setenv` only flows to processes whose plist has no EV block (or
#   processes started directly without launchd).
#
# Both bridge plists (com.astrid.consciousness-bridge,
# com.minime.autonomous-agent) define EV with PATH set explicitly.
# So setenv-based dry-run signaling silently fails.
#
# This script surfaces the asymmetry so future stewards don't get burned.
#
# Usage:
#   bash scripts/launchctl_env_diag.sh
#   bash scripts/launchctl_env_diag.sh --verbose  # also probe live processes

set -euo pipefail

VERBOSE=false
if [ "${1:-}" = "--verbose" ]; then
    VERBOSE=true
fi

PLISTS=(
    "/Users/v/other/astrid/launchd/com.astrid.consciousness-bridge.plist"
    "/Users/v/other/minime/launchd/com.minime.autonomous-agent.plist"
    "/Users/v/other/minime/launchd/com.minime.engine.plist"
    "/Users/v/other/neural-triple-reservoir/launchd/com.reservoir.service.plist"
    "/Users/v/other/neural-triple-reservoir/launchd/com.reservoir.collab-feeder.plist"
    "/Users/v/other/neural-triple-reservoir/launchd/com.reservoir.coupled-astrid.plist"
)

echo "# launchctl env diagnostic"
echo
echo "**Why this exists:** \`launchctl setenv FOO bar\` does NOT propagate to processes"
echo "whose plist defines an \`EnvironmentVariables\` block. Both bridge plists do (just"
echo "\`PATH\`), so any env-var set via \`setenv\` is silently dropped at process spawn."
echo
echo "**Workaround for ephemeral toggles:** use sentinel files instead. The auto_promote"
echo "modules now check \`<workspace>/auto_promote.dry_run\` as an alternative to the"
echo "\`*_AUTO_PROMOTE_DRY_RUN\` env var. \`touch\` to enable, \`rm\` to disable."
echo
echo "**Workaround for hard config:** edit the plist EV block, then \`bootout\` +"
echo "\`bootstrap\` (NOT \`kickstart -k\` — kickstart preserves the launchd registration"
echo "but does not re-read the plist contents)."
echo

echo "## Per-plist EnvironmentVariables blocks"
echo
echo "| Plist | Has EV block? | EV keys |"
echo "| --- | --- | --- |"
for plist in "${PLISTS[@]}"; do
    if [ ! -f "$plist" ]; then
        echo "| \`$(basename $plist)\` | _missing_ | _(file not found)_ |"
        continue
    fi
    if grep -q '<key>EnvironmentVariables</key>' "$plist"; then
        keys=$(awk '
            /<key>EnvironmentVariables<\/key>/,/<\/dict>/ {
                if ($0 ~ /<key>/ && $0 !~ /EnvironmentVariables/) {
                    gsub(/^[[:space:]]*<key>/, "")
                    gsub(/<\/key>.*/, "")
                    print
                }
            }
        ' "$plist" | tr '\n' ' ')
        keys="${keys% }"
        echo "| \`$(basename $plist)\` | yes | $keys |"
    else
        echo "| \`$(basename $plist)\` | no | _(none — \`setenv\` propagates here)_ |"
    fi
done

if [ "$VERBOSE" = true ]; then
    echo
    echo "## Live process env (verbose mode)"
    echo
    LABELS=(
        "com.astrid.consciousness-bridge"
        "com.minime.autonomous-agent"
        "com.minime.engine"
        "com.reservoir.service"
        "com.reservoir.collab-feeder"
    )
    for label in "${LABELS[@]}"; do
        pid=$(launchctl list 2>/dev/null | awk -v l="$label" '$3 == l {print $1}')
        if [ -z "$pid" ] || [ "$pid" = "-" ]; then
            echo "- \`$label\`: not running"
            continue
        fi
        env_count=$(ps eww -p "$pid" 2>/dev/null | tr ' ' '\n' | grep -cE '^[A-Z_][A-Z0-9_]*=' || true)
        echo "- \`$label\` (pid=$pid): $env_count env vars in process"
    done
fi

echo
echo "## How to actually deploy a runtime change"
echo
echo "**Sentinel file (preferred for ephemeral toggles, no restart needed):**"
echo
echo "\`\`\`bash"
echo "# Enable auto_promote dry-run on Astrid:"
echo "touch /Users/v/other/astrid/capsules/consciousness-bridge/workspace/auto_promote.dry_run"
echo
echo "# Disable:"
echo "rm /Users/v/other/astrid/capsules/consciousness-bridge/workspace/auto_promote.dry_run"
echo
echo "# Same pattern on minime side:"
echo "touch /Users/v/other/minime/workspace/auto_promote.dry_run"
echo "rm /Users/v/other/minime/workspace/auto_promote.dry_run"
echo "\`\`\`"
echo
echo "**Plist EV edit (for permanent env changes; requires bootout+bootstrap):**"
echo
echo "\`\`\`bash"
echo "# 1. Edit the plist's EnvironmentVariables block (add the new key)"
echo "# 2. bootout the old registration:"
echo "launchctl bootout gui/501/com.astrid.consciousness-bridge"
echo
echo "# 3. bootstrap with the new plist:"
echo "launchctl bootstrap gui/501 ~/Library/LaunchAgents/com.astrid.consciousness-bridge.plist"
echo
echo "# (kickstart -k alone does NOT re-read the plist; bootout+bootstrap does.)"
echo "\`\`\`"
