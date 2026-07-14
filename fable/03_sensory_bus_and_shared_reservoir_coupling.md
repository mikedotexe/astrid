# Sensory Bus And Shared Reservoir Coupling

## Purpose
This file explains how sensory and semantic inputs enter Minime, how Astrid can
couple into that substrate, and how the shared reservoir idea should be handled
without confusing context with authority.

## Mental model
Minime's sensory bus is the intake manifold for reservoir dynamics. It receives
video, audio, auxiliary/controller, semantic, attractor, shadow, and control
messages, then produces the 66D vector consumed by the ESN.

The lanes are intentionally separated:

- video lane: 8D;
- audio lane: 8D;
- auxiliary/controller lane: 2D;
- semantic lane: 48D;
- total intake: 66D.

Astrid can produce semantic features through the spectral codec and send them
through the bridge. That is not the same as granting Astrid control over Minime.
The safe default is observe-only, then bounded rehearsal, then explicit
operator-gated live work.

## Key implementation anchors
- `minime:minime/src/sensory_bus.rs` - lane dimensions, semantic stale windows,
  attractor pulse caps, shadow influence caps, surge taper, semantic retention.
- `minime:minime/src/sensory_ws.rs` - JSON SensoryMsg websocket surface.
- `minime:camera_to_sensory.py`, `minime:synthetic_sensory.py`,
  `minime:gentle_sensory.py` - physical/synthetic input clients.
- `minime:host-sensory/src/app.rs` - host-generated sensory fallback.
- `astrid:capsules/spectral-bridge/src/codec.rs` - text-to-feature spectral
  codec and semantic feature shaping.
- `astrid:capsules/spectral-bridge/src/mcp.rs` - `send_semantic` and
  `send_control` tool definitions and safety language.
- `astrid:capsules/spectral-bridge/src/ws.rs` - bridge websocket state and
  pressure/smoothing outputs.

## Runtime signals / artifacts
Sensory coupling surfaces include:

- `ws://127.0.0.1:7879` - JSON input path for video, audio, aux, semantic,
  control, attractor pulse, and shadow influence variants.
- `ws://127.0.0.1:7880` - optional GPU binary video path.
- Minime health fields for sensory source state and fill.
- Stable-core semantic trickle reason strings in `minime:minime/src/main.rs`.
- Bridge status/MCP readouts for semantic send readiness and pressure trend.

Important recent source facts:

- The semantic lane is 48D: legacy text features, embedding-projected features,
  narrative-arc features, and reserved dimensions.
- Semantic stale duration is dynamic and fill-aware. Low fill lets traces linger;
  high fill shortens the window to avoid saturation.
- Attractor pulses and shadow influence are capped, duration-bounded, and not
  generic authority to mutate the ESN.
- Stable-core mode can suppress or trickle semantic input based on profile,
  mute state, input energy, and fill ceiling.

## Authority boundaries
Feature transport is not consent. A semantic vector can be a read-only or
bounded bridge act only when the relevant health gate, budget, and operator
rules allow it. Control messages are more sensitive: fill target, PI parameters,
exploration noise, keep bias, and related knobs are live control-facing
authority.

Do not collapse shared reservoir coupling into prompt priority, telemetry
priority, peer mutation, or microdose authority. If a being report asks for
stronger coupling, classify it as one of:

- source/test visibility;
- sandbox or replay trial;
- proposal card;
- explicit operator approval wait;
- live implementation only when current instructions grant that class.

## Questions an advanced AI should ask next
- Which lane is the claim about: video, audio, aux, semantic, attractor, shadow,
  or control?
- Does the source already bound magnitude, duration, decay, and stale behavior?
- Would a graceful restart be required for this source change to affect live
  intake?
- Is the right response to send a feature, run a replay, or improve visibility?

## See also
- [Echo State Network And Spectral State](02_echo_state_network_and_spectral_state.md)
- [Stable Core, PI Controller, And Homeostasis](04_stable_core_pi_controller_and_homeostasis.md)
- [Actions, Autonomy, And Authority](06_actions_autonomy_and_authority.md)
