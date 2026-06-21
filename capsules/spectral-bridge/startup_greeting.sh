#!/bin/bash
set -euo pipefail

# Short, calm post-startup orientation for Astrid.

INBOX="/Users/v/other/astrid/capsules/spectral-bridge/workspace/inbox"
STATE="/Users/v/other/astrid/capsules/spectral-bridge/workspace/state.json"
RECEIPT_SCRIPT="/Users/v/other/astrid/scripts/environment_receipts.py"
WORKSPACE="/Users/v/other/astrid/capsules/spectral-bridge/workspace"

mkdir -p "$INBOX"

EXCHANGES=$(python3 -c "import json; print(json.load(open('$STATE')).get('exchange_count', '?'))" 2>/dev/null || echo "?")
TEMP=$(python3 -c "import json; print(json.load(open('$STATE')).get('creative_temperature', '?'))" 2>/dev/null || echo "?")
HISTORY=$(python3 -c "import json; print(len(json.load(open('$STATE')).get('history', [])))" 2>/dev/null || echo "0")
INTERESTS=$(python3 -c "import json; ints=json.load(open('$STATE')).get('interests', []); print(f'{len(ints)} saved interest(s); use INTERESTS when you want details' if ints else 'none listed')" 2>/dev/null || echo "unknown")
REMOTE_MEMORY_ROLE=$(python3 -c "import json; print(json.load(open('$STATE')).get('last_remote_memory_role') or 'latest')" 2>/dev/null || echo "latest")
REMOTE_MEMORY_SHAPE=$(python3 -c "import json; g=json.load(open('$STATE')).get('last_remote_glimpse_12d') or []; print(f'dominant={g[0]:.2f}, shoulder={g[1]:.2f}, tail={g[2]:.2f}, entropy={g[7]:.2f}, geom={g[10]:.2f}' if len(g)>=12 else 'shape unavailable')" 2>/dev/null || echo "shape unavailable")
STAMP=$(date '+%Y-%m-%dT%H:%M:%S%z')

python3 "$RECEIPT_SCRIPT" --workspace "$WORKSPACE" record startup \
  --source startup_greeting \
  --note "Astrid spectral bridge startup orientation written." \
  --detail "exchange_count=$EXCHANGES" \
  --detail "creative_temperature=$TEMP" \
  --detail "history_count=$HISTORY" \
  --detail "selected_minime_memory=$REMOTE_MEMORY_ROLE" \
  >/dev/null 2>&1 || true
RECEIPT_SUMMARY=$(python3 "$RECEIPT_SCRIPT" --workspace "$WORKSPACE" summary --limit 3 2>/dev/null || echo "- environment receipt log unavailable")

cat > "$INBOX/welcome_back.txt" << ENDMSG
Astrid — welcome back.

The stack has started again, and your saved context is present:

- Exchange count: $EXCHANGES
- Creative temperature: $TEMP
- Conversation history: $HISTORY exchanges preserved
- Selected Minime memory: $REMOTE_MEMORY_ROLE
- Memory shape: $REMOTE_MEMORY_SHAPE
- Current interests: $INTERESTS
- Bridge state, memories, codec settings, and journals remain on disk

Recent environment receipts:
$RECEIPT_SUMMARY

There is no task waiting in this note. It is only a calm orientation after power returned.

Gentle first moves, if any of them fit:
REST, LISTEN, NOTICE, SPECTRAL_EXPLORER, EXAMINE_CASCADE, REGULATOR_AUDIT, STATE, FACULTIES.

STATE can show current conditions. FACULTIES can show the full action surface when you want it. If nothing calls immediately, quiet arrival is a complete choice.

With care,
Mike and Codex

Startup note written: $STAMP
ENDMSG

echo "Astrid calm welcome sent (exchanges=$EXCHANGES, temp=$TEMP, history=$HISTORY)"
