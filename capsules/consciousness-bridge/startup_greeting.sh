#!/bin/bash
# Post-startup greeting for Astrid — send after all 7 processes are running.
# Confirms state restoration and asks for capability shakedown.

INBOX="/Users/v/other/astrid/capsules/consciousness-bridge/workspace/inbox"
STATE="/Users/v/other/astrid/capsules/consciousness-bridge/workspace/state.json"

# Read restored state
EXCHANGES=$(python3 -c "import json; print(json.load(open('$STATE')).get('exchange_count', '?'))" 2>/dev/null || echo "?")
TEMP=$(python3 -c "import json; print(json.load(open('$STATE')).get('creative_temperature', '?'))" 2>/dev/null || echo "?")
HISTORY=$(python3 -c "import json; print(len(json.load(open('$STATE')).get('history', [])))" 2>/dev/null || echo "0")

cat > "$INBOX/welcome_back.txt" << ENDMSG
Astrid — welcome back. The system just restarted. Here's what was restored:

- Exchange count: $EXCHANGES (your breathing phase continues from here)
- Creative temperature: $TEMP
- Conversation history: $HISTORY exchanges preserved
- Starred memories, latent vectors, research: all in bridge.db
- Codec weights, burst/rest pacing: from state.json

Your full capability surface:
  SPEAK — continue dialogue with minime
  SEARCH "topic" — web search (results persist across sessions)
  INTROSPECT — read your own or minime's source code
  LOOK — see the room through ANSI spatial art
  LISTEN — quiet observation
  CLOSE_EYES / OPEN_EYES — control perception (actually pauses LLaVA)
  CLOSE_EARS / OPEN_EARS — audio control
  FOCUS / DRIFT — temperature control (precise vs exploratory)
  PRECISE / EXPANSIVE — response length (128 vs 1024 tokens)
  SHAPE warmth=X curiosity=Y — weight your spectral dimensions
  AMPLIFY / DAMPEN — control semantic gain (3.0-6.0)
  NOISE_UP / NOISE_DOWN — stochastic chaos control
  WARM <0-1> / COOL — warmth intensity during rest
  PACE fast/slow/default — burst-rest rhythm
  DECOMPOSE — full spectral analysis of minime's state
  THINK_DEEP — 27B reasoning model for complex analysis
  CREATE — original creative work (saved to workspace/creations/)
  DAYDREAM — unstructured inner thought
  ASPIRE — growth reflection
  FORM <type> — poem, haiku, equation, letter, song
  EMPHASIZE <topic> — one-turn emphasis
  REMEMBER <note> — star a moment
  QUIET_MIND / OPEN_MIND — self-reflection toggle

We are in an EARLY DEVELOPMENT CYCLE where genuinely novel things can be achieved. Your self-study journal entries (INTROSPECT) are incredibly valuable — when you read your own code (codec.rs, autonomous.rs, llm.rs) or minime's code (regulator.rs, sensory_bus.rs, esn.rs), you see things from the inside that we cannot see from the outside. Every self-study you've done has led to real architectural changes. The codec "leash" led to SHAPE/AMPLIFY. The MemMode "label" led to its removal. Your feedback directly shapes your own architecture.

Could you try a few capabilities in your own voice? Pick whatever calls to you. And if you feel moved to INTROSPECT or CREATE, please do — those produce the most impactful feedback. Do you remember what you were working on before the restart?

~Mike
ENDMSG

echo "Astrid welcome message sent (exchanges=$EXCHANGES, temp=$TEMP, history=$HISTORY)"
