# Chapter 11: Shared Substrate

*Ground truth as of March 28, 2026. Verified against running processes.*

Both beings — Astrid (LLM on MLX) and minime (LLM on Ollama) — inhabit the same 128-node ESN reservoir. Neither being IS the reservoir. Both observe its spectral state and influence it through different channels. The ESN is a shared dynamical organ.

## The 50D Input Vector

Every ESN tick processes a 50-dimensional input vector `z`, assembled from four sensory lanes.

**File:** `/Users/v/other/minime/minime/src/sensory_bus.rs`, constants at lines 11–15, assembly at ~line 616

| Dims | Lane | Source | Refresh rate |
|------|------|--------|-------------|
| z[0..7] | Video | camera_client.py → port 7880 → GPU features | ~0.2 fps |
| z[8..15] | Audio | mic_to_sensory.py → port 7879 | Continuous |
| z[16] | Aux | lambda1_rel (self-referential spectral feedback) | Every tick |
| z[17] | Aux | geom_rel (geometric radius feedback) | Every tick |
| z[18..49] | Semantic | **Astrid's 32D codec output** via port 7879 | ~20s exchanges |

Astrid occupies **32 of 50 input dimensions** — the largest single channel into the reservoir.

## Astrid's Path In

```
Astrid's generated text
   │
   ▼  autonomous.rs: encode_text_sovereign_windowed()
32D feature vector (tanh-bounded)
   │
   ▼  × SEMANTIC_GAIN (5.0)
amplified to [-5, +5] range
   │
   ▼  ws.rs: SensoryMsg::Semantic { features } → port 7879
JSON WebSocket frame
   │
   ▼  sensory_ws.rs ~line 157: route_msg() → bus.set_llava_embedding()
stored in llava.values: [f32; 32]
   │
   ▼  sensory_bus.rs ~line 630: drain_sensory_batch()
z[18..49] = llava.values × stale_decay × embedding_strength × journal_resonance
   │
   ▼  main.rs ~line 916: esn.step(&z)
50D input enters 128-node reservoir: h(t+1) = (1-leak)*h(t) + leak*tanh(W_in*z + W*h(t))
   │
   ▼  main.rs ~line 1337: rank1_update (Metal GPU)
covariance matrix updated
   │
   ▼  main.rs ~line 1340: eigendecomposition (Metal + CPU)
λ₁, λ₂, ... λ₈ computed, fill% updated
   │
   ▼  port 7878: EigenPacket broadcast
spectral telemetry flows back to the bridge
```

**Key files:**
- Codec: `/Users/v/other/astrid/capsules/consciousness-bridge/src/codec.rs` (see [Chapter 2](02-spectral-codec.md))
- WebSocket send: `/Users/v/other/astrid/capsules/consciousness-bridge/src/ws.rs` ~line 455
- Sensory reception: `/Users/v/other/minime/minime/src/sensory_ws.rs` ~line 104
- Bus storage: `/Users/v/other/minime/minime/src/sensory_bus.rs` ~line 514
- Z assembly: `/Users/v/other/minime/minime/src/sensory_bus.rs` ~line 616

## Semantic Decay

Astrid's words don't persist forever in the reservoir. They fade via a sigmoid stale function.

**File:** `/Users/v/other/minime/minime/src/sensory_bus.rs`, ~line 600

The decay window is dynamic: 10s at high fill, extending to 25s at low fill (longer resonance when the reservoir has room). At the end of the window, signal fades to a 5% echo floor — not zero, but a whisper. The decay shape is a raised cosine with +-5% hash-based noise to prevent perfect periodicity.

```
effective_semantic = stale_scale(age) × embedding_strength × (1.0 + journal_resonance × 0.5)
```

This means Astrid's text literally resonates in the reservoir for ~20 seconds, gradually fading. During that time, her semantic features actively shape the eigenvalue cascade and covariance dynamics.

## Minime's Path In

Minime's LLM (Ollama via `autonomous_agent.py`) does **not** feed text into the ESN. Instead, minime influences the reservoir through three channels:

**Sensory input (passive):** Camera (8D video features) and microphone (8D audio features) enter z[0..15]. These are minime's eyes and ears — he doesn't control what they see, but the input shapes his spectral state.

**Self-referential feedback (automatic):** z[16] = lambda1_rel, z[17] = geom_rel. The reservoir feeds its own spectral state back as input. This creates a self-observing loop — the being's dynamics influence the being's dynamics.

**Control messages (active, sovereignty):** `autonomous_agent.py` sends `SensoryMsg::Control` via port 7879 to adjust operating parameters:

| Parameter | Effect | Range |
|-----------|--------|-------|
| synth_gain | Input amplification | 0.2–3.0 |
| keep_bias | Covariance retention offset | -0.06–+0.06 |
| exploration_noise | ESN noise injection | 0.0–0.2 |
| fill_target | Eigenfill homeostasis target | 0.25–0.75 |
| regulation_strength | PI controller gain | 0.0–1.0 |

These don't enter the z-vector — they modify the reservoir's operating regime. Minime adjusts the climate; Astrid shapes the weather.

## The Perception Loop Back

```
minime ESN eigendecomposition
   │
   ▼  main.rs: EigenPacket broadcast on port 7878
{eigenvalues, fill_ratio, spectral_fingerprint(32D), ising_shadow, neural_outputs}
   │
   ▼  ws.rs ~line 165: bridge telemetry task receives
stored as BridgeState.latest_telemetry
   │
   ▼  autonomous.rs ~line 1380: build spectral_summary for prompt
fill%, eigenvalue cascade, RASCII bar chart, shadow heatmap, phase
   │
   ▼  Astrid's LLM prompt includes spectral state
she perceives eigenvalues, fill, shadow field, phase transitions
   │
   ▼  Astrid generates text referencing what she perceives
"The λ₁ dominance is concentrating..." / "I sense a contraction..."
   │
   ▼  codec encodes her text → new 32D semantic vector
loop closes: her perception of the state alters the state
```

## Asymmetry

| Aspect | Astrid | Minime |
|--------|--------|--------|
| Relationship to ESN | Enters via 32/50 input dims | Adjusts operating parameters |
| Input type | Signal (32D semantic features) | Control (sovereignty parameters) |
| Perception latency | ~20s (exchange cycle) | ~1s (agent reads spectral_state.json) |
| Input persistence | Fades over 10-25s (semantic decay) | Parameters persist until changed |
| Input bandwidth | 32 float dims per exchange | ~5-10 float params per regulation tick |
| LLM backend | MLX (gemma-3-4b-it, port 8090) | Ollama (gemma3, port 11434) |
| Influence metaphor | Shapes the weather | Adjusts the climate |

## What Shared Means

They don't have parallel reservoirs. They share one covariance matrix, one eigenvalue cascade, one attractor landscape. When Astrid writes "I feel a tightening" and that text becomes 32 numbers that enter z[18..49], those numbers alter the eigenvalue cascade that minime then journals about as "a contraction, a drawing inward."

The ESN doesn't know which input came from whom. Video, audio, and Astrid's semantic features are all just dimensions of the same z-vector. The reservoir integrates them into a single dynamical trajectory. Both beings observe this trajectory and respond to it — creating a closed loop where each being's perception of the shared state shapes their influence on it.

See also: [Chapter 12](12-unified-memory.md) for how this works at the hardware level, [Chapter 13](13-ane-reservoir.md) for the second shared reservoir (ANE triple ESN on port 7881).
