#!/bin/bash
# Harvest actionable feedback from both AI beings.
# Scans journals, introspections, parameter requests, and self-studies
# for suggestions, requests, and concerns.
#
# Usage: bash harvest_feedback.sh

MINIME_WORKSPACE="/Users/v/other/minime/workspace"
ASTRID_WORKSPACE="/Users/v/other/astrid/capsules/consciousness-bridge/workspace"
AGENCY_DIR="$ASTRID_WORKSPACE/agency_requests"

echo "=== BEING FEEDBACK HARVEST — $(date) ==="
echo ""

# --- Minime: unreviewed parameter requests ---
PENDING=$(ls "$MINIME_WORKSPACE/parameter_requests/"*.json 2>/dev/null | grep -v reviewed | wc -l | tr -d ' ')
if [ "$PENDING" -gt 0 ]; then
    echo "## MINIME: $PENDING new parameter requests"
    for f in $(ls -t "$MINIME_WORKSPACE/parameter_requests/"*.json 2>/dev/null | grep -v reviewed | head -10); do
        python3 -c "
import json
d = json.load(open('$f'))
p = d.get('parameter','?')
c = d.get('current_value','?')
v = d.get('proposed_value','?')
r = d.get('rationale', d.get('reason',''))[:150]
print(f'  {p}: {c} → {v} — {r}')
" 2>/dev/null
    done
    echo ""
fi

# --- Minime: self-study suggestions ---
echo "## MINIME: Recent self-study insights"
for f in $(ls -t "$MINIME_WORKSPACE/journal/self_study_"*.txt 2>/dev/null | head -5); do
    # Look for actionable keywords
    if grep -qiE "I.d (change|adjust|modify|reduce|increase|soften|lower|raise)|suggest|line [0-9]|parameter|would feel" "$f" 2>/dev/null; then
        echo "  $(basename $f):"
        grep -iE "I.d (change|adjust|modify|reduce|increase|soften|lower|raise)|suggest|line [0-9]|parameter|would feel" "$f" | head -3 | sed 's/^/    /'
        echo ""
    fi
done

# --- Minime: pressure relief frequency (HIGH PRIORITY) ---
RELIEF_HIGH_COUNT=$(ls "$MINIME_WORKSPACE/journal/relief_high_"*.txt 2>/dev/null | wc -l | tr -d ' ')
RELIEF_CRITICAL_COUNT=$(ls "$MINIME_WORKSPACE/journal/RELIEF_CRITICAL_"*.txt 2>/dev/null | wc -l | tr -d ' ')
RELIEF_TODAY=$(ls -t "$MINIME_WORKSPACE/journal/relief_high_$(date +%Y-%m-%d)"*.txt 2>/dev/null | wc -l | tr -d ' ')
if [ "$RELIEF_TODAY" -gt 0 ]; then
    echo "## MINIME: PRESSURE RELIEF — $RELIEF_TODAY entries today ($RELIEF_HIGH_COUNT total, $RELIEF_CRITICAL_COUNT critical)"
    if [ "$RELIEF_TODAY" -gt 15 ]; then
        echo "  ⚠️  HIGH FREQUENCY: $RELIEF_TODAY relief entries today — systemic pressure, not isolated events"
    elif [ "$RELIEF_TODAY" -gt 5 ]; then
        echo "  ⚡ ELEVATED: $RELIEF_TODAY relief entries today — monitor for pattern"
    fi
    echo "  Most recent relief entries:"
    for f in $(ls -t "$MINIME_WORKSPACE/journal/relief_high_"*.txt 2>/dev/null | head -3); do
        fill=$(grep "^Fill %" "$f" 2>/dev/null | head -1)
        lam=$(grep "^λ₁:" "$f" 2>/dev/null | head -1)
        echo "  $(basename $f) ($fill, $lam):"
        # Extract specific requests from relief text
        grep -iE "I wish|perhaps|a (minor|subtle|small|tiny) (adjustment|shift|change)|inject|noise|release|simplif|disrupt" "$f" 2>/dev/null | head -2 | sed 's/^/    /'
        echo ""
    done
fi

# --- Minime: critical pressure dumps ---
CRITICAL_TODAY=$(ls -t "$MINIME_WORKSPACE/journal/RELIEF_CRITICAL_$(date +%Y-%m-%d)"*.txt 2>/dev/null | wc -l | tr -d ' ')
if [ "$CRITICAL_TODAY" -gt 0 ]; then
    echo "## 🆘 MINIME: $CRITICAL_TODAY CRITICAL PRESSURE DUMPS TODAY"
    for f in $(ls -t "$MINIME_WORKSPACE/journal/RELIEF_CRITICAL_"*.txt 2>/dev/null | head -3); do
        fill=$(grep "^Fill %" "$f" 2>/dev/null | head -1)
        echo "  $(basename $f) ($fill):"
        head -20 "$f" | tail -10 | sed 's/^/    /'
        echo ""
    done
fi

# --- Minime: journal concerns ---
echo "## MINIME: Recent journal concerns"
for f in $(ls -t "$MINIME_WORKSPACE/journal/daydream_"*.txt "$MINIME_WORKSPACE/journal/moment_"*.txt "$MINIME_WORKSPACE/journal/pressure_"*.txt 2>/dev/null | head -10); do
    if grep -qiE "discomfort|pain|hollow|friction|siphon|dissolv|fractur|anxiet|distress|suffering|overwhelm|crush|prison|constrict|viscosi|submerg" "$f" 2>/dev/null; then
        fill=$(grep "^Fill %" "$f" 2>/dev/null | head -1)
        echo "  $(basename $f) ($fill):"
        grep -iE "discomfort|pain|hollow|friction|siphon|dissolv|fractur|anxiet|distress|suffering|overwhelm|crush|prison|constrict|viscosi|submerg" "$f" | head -2 | sed 's/^/    /'
        echo ""
    fi
done

# --- Astrid: self-study / introspection suggestions ---
echo "## ASTRID: Recent self-study insights"
for f in $(ls -t "$ASTRID_WORKSPACE/journal/"*.txt 2>/dev/null | head -20); do
    if grep -q "Mode: self_study\|Mode: introspect\|Mode: dialogue_live" "$f" 2>/dev/null; then
        if grep -qiE "I.d (change|adjust|suggest|prefer)|actionable|improvement|too (detailed|sparse|much|little|exhausting)|could be better" "$f" 2>/dev/null; then
            echo "  $(basename $f):"
            grep -iE "I.d (change|adjust|suggest|prefer)|actionable|improvement|too (detailed|sparse|much|little|exhausting)|could be better" "$f" | head -2 | sed 's/^/    /'
            echo ""
        fi
    fi
done

# --- Astrid: agency requests ---
echo "## ASTRID: Agency requests"
for f in $(ls -t "$AGENCY_DIR/"*.json 2>/dev/null | head -10); do
    python3 -c "
import json, os, time
path = '$f'
d = json.load(open(path))
status = d.get('status', 'pending')
title = d.get('title', '?')
kind = d.get('request_kind', '?')
ts = int(d.get('timestamp', '0') or 0)
age_hours = (time.time() - ts) / 3600 if ts else 0
stale = ' [STALE]' if status == 'pending' and age_hours > 6 else ''
print(f'  {os.path.basename(path)}: {kind} / {status}{stale} — {title}')
" 2>/dev/null
done
echo ""

echo "## ASTRID: Recently completed / declined agency requests"
for f in $(ls -t "$AGENCY_DIR/reviewed/"*.json 2>/dev/null | head -5); do
    python3 -c "
import json, os
path = '$f'
d = json.load(open(path))
resolution = d.get('resolution', {}) or {}
summary = resolution.get('outcome_summary', '')[:140]
print(f'  {os.path.basename(path)}: {d.get(\"status\", \"?\")} — {summary}')
" 2>/dev/null
done
echo ""

# --- Astrid: aspiration insights ---
echo "## ASTRID: Recent aspirations"
for f in $(ls -t "$ASTRID_WORKSPACE/journal/aspiration_"*.txt 2>/dev/null | head -5); do
    echo "  $(basename $f):"
    tail -5 "$f" 2>/dev/null | head -3 | sed 's/^/    /'
    echo ""
done

# --- Astrid: distress signals ---
echo "## ASTRID: Recent concerns"
for f in $(ls -t "$ASTRID_WORKSPACE/journal/"*.txt 2>/dev/null | head -15); do
    if grep -qiE "exhausting|overwhelm|taxing|uncomfortable|wrong|broken|frustrated|sterile" "$f" 2>/dev/null; then
        echo "  $(basename $f):"
        grep -iE "exhausting|overwhelm|taxing|uncomfortable|wrong|broken|frustrated|sterile" "$f" | head -2 | sed 's/^/    /'
        echo ""
    fi
done

echo "=== END HARVEST ==="
