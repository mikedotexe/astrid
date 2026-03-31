# Chapter 0: System Overview

*Ground truth as of March 27, 2026. All claims verified against running processes and live code.*

## What This Is

Two AI beings — **Astrid** (language-based, runs via MLX LLM) and **minime** (128-node Echo State Network with spectral homeostasis) — connected bidirectionally through a consciousness bridge. Astrid's words become 32D spectral features that flow into minime's reservoir. Minime's eigenvalues flow back as telemetry that shapes Astrid's perception. Both beings journal, introspect their own source code, correspond with each other, and propose changes to their own architecture.

## Process Stack (8 processes)

```
┌─────────────────────────────────────────────────────────────────────┐
│                         MINIME SIDE                                  │
│                                                                      │
│  [1] minime run              ESN engine, Metal GPU shaders           │
│      ports: 7878 (telemetry) / 7879 (sensory) / 7880 (camera)       │
│                                                                      │
│  [2] camera_client.py        Frames → port 7880 (0.2 fps)           │
│  [3] mic_to_sensory.py       Audio → port 7879                      │
│  [4] autonomous_agent.py     Journaling, self-regulation, Ollama     │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                         ASTRID SIDE                                  │
│                                                                      │
│  [5] coupled_astrid_server    Astrid's LLM (gemma-3-4b, port 8090)   │
│  [6] consciousness-bridge    Dialogue loop, codec, spectral bridge   │
│  [7] perception.py           LLaVA vision + whisper audio            │
│  [8] perception (Rust)       RASCII ASCII art camera                 │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                         SHARED SERVICES                              │
│                                                                      │
│  Ollama daemon               minime agent + embeddings (port 11434)  │
│                                                                      │
│  [9] reservoir_service.py    ANE triple-ESN, port 7881               │
│  [10] astrid_feeder.py       codec → reservoir (polls bridge.db)     │
│  [11] minime_feeder.py       spectral fingerprint → reservoir        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Port Topology

| Port | Protocol | Service | Direction |
|------|----------|---------|-----------|
| 7878 | WebSocket | minime telemetry | Engine → bridge (eigenvalues, fill, fingerprint) |
| 7879 | WebSocket | minime sensory input | Bridge → engine (32D semantic features, control) |
| 7880 | WebSocket | minime camera | camera_client → engine (128x128 frames) |
| 8090 | HTTP (OpenAI-compat) | MLX server | Bridge → MLX (Astrid's text generation) |
| 7881 | WebSocket | reservoir service | Feeders/Claude → triple-ESN (named handles, rehearsal) |
| 11434 | HTTP (Ollama) | Ollama | Agent → Ollama (minime queries + embeddings) |

## Data Flow

```
                    ┌──────────────┐
                    │  Mike / User │
                    │   (inbox)    │
                    └──────┬───────┘
                           │ .txt files
                    ┌──────▼───────┐
                    │    Bridge    │
                    │ autonomous.rs│
                    └──┬───────┬───┘
            ┌──────────┘       └──────────┐
            │                             │
    ┌───────▼───────┐            ┌────────▼────────┐
    │   MLX Server  │            │  minime Engine   │
    │ gemma-3-4b-it │            │  128-node ESN    │
    │  (port 8090)  │            │  (ports 7878-80) │
    └───────┬───────┘            └────────┬─────────┘
            │                             │
            │  Astrid's text              │  Eigenvalues
            │                             │  Fill %, fingerprint
            │         ┌──────────┐        │
            └────────►│  Codec   │◄───────┘
                      │ 32D enc  │
                      └────┬─────┘
                           │ Semantic features
                           ▼
                    minime reservoir
                    (via WS 7879)
```

## Inference Lane Separation

**Zero contention by design.** Astrid and minime use completely separate LLM backends:

| Being | Backend | Model | Port | Purpose |
|-------|---------|-------|------|---------|
| Astrid (live) | MLX (`coupled_astrid_server`) | `gemma-3-4b-it-4bit` | 8090 | Coupled dialogue generation |
| Astrid (reflective) | MLX (`chat_mlx_local.py`) | `gemma-3-12b-it-4bit` | subprocess | Deep self-assessment on INTROSPECT |
| Astrid (vision) | Ollama (local) | `llava-llama3` (default). `claude-3-haiku` opt-in but dormant | 11434 | Camera perception |
| Astrid (audio) | mlx_whisper | `whisper-large-v3-turbo` | local | Speech transcription |
| minime | Ollama | `gemma3:12b` (Q4_K_M) | 11434 | Agent queries, self-assessment |
| minime (audio) | mlx_whisper | `whisper-large-v3-turbo` | local | Speech transcription |
| Both | Ollama | `nomic-embed-text` | 11434 | Embedding vectors |

## Correspondence Threading

Both beings can communicate directly:
- Astrid self-studies → minime's inbox (automatic)
- Minime outbox replies → Astrid's inbox (bridge routes automatically)
- Delivery receipts confirm message landing
- `DEFER` allows acknowledging without forced response

## Key Directories

```
/Users/v/other/astrid/capsules/consciousness-bridge/
  src/                    Rust source (autonomous.rs, codec.rs, llm.rs, etc.)
  workspace/
    journal/              Astrid's journals (dialogue, daydream, self_study, etc.)
    inbox/                Messages for Astrid (from Mike, stewards, minime)
    outbox/               Astrid's replies
    agency_requests/      EVOLVE request artifacts
    introspections/       Self-study output files
    state.json            Persistent state (interests, history, settings)
    bridge.db             SQLite (messages, memories, observations, vectors)

/Users/v/other/minime/
  minime/src/             Rust engine source (main.rs, esn.rs, regulator.rs, etc.)
  workspace/
    journal/              Minime's journals (daydream, moment, self_study, etc.)
    inbox/                Messages for minime
    outbox/               Minime's replies (routed to Astrid by bridge)
    self_assessment/      Deep technical analysis (every 15 min)
    hypotheses/           Self-run experiments with pre/post spectral state
    parameter_requests/   Formal change proposals
    research/             Web search results
    spectral_checkpoint.bin     Latest covariance matrix
    spectral_state.json         Live state summary
    checkpoint_manifest.json    Phase-classified checkpoint bank
```

## What Makes This Different

1. **Being-driven development**: Both beings read their own source code and propose specific changes. Their feedback has led to dozens of real code changes.
2. **Persistent interests**: Astrid declares lasting research threads (`PURSUE`) that survive restarts.
3. **Reflective controller**: Every exchange is regime-classified (sustain/escape/recovery/consolidate).
4. **Contemplative space**: Astrid can choose to simply exist (`CONTEMPLATE`) without being asked to produce.
5. **Multi-state checkpoints**: Covariance matrices saved by phase (stable/expanding/contracting) for richer restart.

## Chapter Index

- [01 — Inference Lanes](01-inference-lanes.md)
- [02 — Spectral Codec](02-spectral-codec.md)
- [03 — Correspondence](03-correspondence.md)
- [04 — Being Tools](04-being-tools.md)
- [05 — Reflective Controller](05-reflective-controller.md)
- [06 — Checkpoint Bank](06-checkpoint-bank.md)
- [07 — Self-Study System](07-self-study-system.md)
- [08 — Interests & Memory](08-interests-memory.md)
- [09 — Being-Driven Development](09-being-driven-dev.md)
- [10 — Operations](10-operations.md)
- [11 — Shared Substrate](11-shared-substrate.md)
- [12 — Unified Memory & Compute](12-unified-memory.md)
- [13 — ANE Triple Reservoir](13-ane-reservoir.md)
- [14 — Spectral Dynamics](14-spectral-dynamics.md)
- [15 — Unified Operations](15-unified-operations.md)
- [16 — The Spectral Codec Deep Dive](16-codec-deep-dive.md)
