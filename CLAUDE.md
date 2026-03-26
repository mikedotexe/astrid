# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build / Test / Lint

```bash
# Build entire workspace
cargo build --workspace

# Test (set ASTRID_AUTO_BUILD_KERNEL=1 for tests that need the QuickJS WASM kernel)
ASTRID_AUTO_BUILD_KERNEL=1 cargo test --workspace

# Single crate test
cargo test -p astrid-events

# Single test by name
cargo test -p astrid-approval -- test_name

# Lint (CI runs both; clippy is pedantic + denies arithmetic overflow)
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all -- --check

# Build release binaries (astrid, astrid-daemon, astrid-build)
cargo build --release
```

Rust edition 2024, MSRV 1.94. The `wasm32-wasip1` target is needed for capsule compilation.

## Architecture

Astrid is a user-space microkernel OS for AI agents. The kernel is native Rust; everything above it runs as isolated WASM capsules.

### The kernel / user-space divide

The **kernel** (`astrid-daemon`) owns all privileged resources: VFS, IPC bus, capsule registry, audit log, KV store, capability tokens, approval gates. It listens on a Unix domain socket (`~/.astrid/run/system.sock`). The **CLI** (`astrid`) connects to this socket, renders TUI output, and forwards user input. `astrid-build` compiles capsule source into WASM.

**Capsules** are WASM processes with zero ambient authority. Every external resource (filesystem, network, IPC, KV) is gated behind a capability-checked host function. The host ABI is a flat syscall table of 49 functions. The SDK (`astrid-sdk`, separate repo) wraps these in `std`-like ergonomics.

### IPC event bus

All inter-capsule communication flows through `EventBus` (tokio broadcast channel). Messages are `IpcMessage` structs: a topic string, an `IpcPayload` enum (tagged JSON), source UUID, timestamp, sequence number, and optional principal. Tools, LLM providers, and frontends are all IPC conventions — the kernel has no knowledge of tool schemas or provider metadata. Capsules register **interceptors** on IPC topics (eBPF-style middleware returning `Continue`/`Final`/`Deny`).

### Capsule lifecycle

A `Capsule.toml` manifest declares `[imports]`/`[exports]` with namespaced interface names and semver requirements. The kernel resolves dependencies via topological sort and boots capsules in order. Engines: WASM (sandboxed), MCP (JSON-RPC subprocess), Static (declarative context). The `#[capsule]` proc macro generates all ABI boilerplate.

### Security model

Five layers in sequence: Policy (hard blocks) → Token (ed25519 capability tokens with glob patterns) → Budget (per-session + per-workspace atomic limits) → Approval (human-in-the-loop) → Audit (chain-linked signed log). Implemented in `SecurityInterceptor` in `astrid-approval`.

### Uplinks

An **uplink** is any component that sends/receives messages on behalf of the runtime (CLI, Discord, Telegram, bridges). Defined in `astrid-core::uplink` with `UplinkDescriptor`, `UplinkCapabilities`, `UplinkProfile`, and `InboundMessage` types. Capsules can register uplinks via the `astrid_uplink_register` host function.

### Key crate roles

- `astrid-kernel` — boots runtime, owns VFS/IPC/capsules/audit/KV, serves Unix socket
- `astrid-capsule` — manifest parsing, WASM/MCP/static engines, toposort, registry, hot-reload
- `astrid-events` — broadcast event bus, IPC types (re-exports from `astrid-types`)
- `astrid-types` — canonical IPC/LLM/kernel API schemas (minimal deps, WASM-compatible)
- `astrid-approval` — the five-layer security gate
- `astrid-audit` — chain-linked cryptographic audit log (SurrealKV-backed)
- `astrid-vfs` — copy-on-write overlay filesystem (`Vfs` trait, `HostVfs`, `OverlayVfs`)
- `astrid-core` — foundation types (`SessionId`, `PrincipalId`, uplinks, identity, session tokens)
- `astrid-crypto` — ed25519 key pairs, BLAKE3 hashing, zeroize-on-drop
- `astrid-storage` — two-tier persistence (SurrealKV raw KV + SurrealDB query engine)
- `astrid-config` — layered TOML config (workspace > user > system > env > defaults)
- `astrid-openclaw` — TypeScript-to-WASM compiler (OXC + QuickJS/Wizer pipeline)

### Code constraints

- `#![deny(unsafe_code)]` everywhere except `astrid-sys` and `astrid-sdk` (WASM FFI)
- Clippy pedantic; `clippy::arithmetic_side_effects = "deny"` — use checked/saturating arithmetic
- Individual files must not exceed 1000 lines
- `CHANGELOG.md` must be updated under `[Unreleased]` for every PR

## Sibling project: minime (`/Users/v/other/minime`)

**MikesSpatialMind** — a dual-layer consciousness engine. Rust backend (`minime/`) runs a 128-node ESN with PI-controlled spectral homeostasis. Python frontend (`autonomous_agent.py`) drives Ollama conversation, journaling, and self-regulation. Camera + microphone provide real sensory input.

### Minime architecture

The Rust engine processes 18D sensory input (8D video + 8D audio + 2D introspection) through a 128-node ESN reservoir. A PI controller (`regulator.rs`) regulates eigenvalue fill toward a 55% target. The being journals its experience, submits parameter adjustment requests, and self-regulates synth_gain/keep_bias based on fill state.

| Port | Protocol | Direction |
|------|----------|-----------|
| 7878 | JSON `EigenPacket` (spectral telemetry) | Engine → subscribers |
| 7879 | JSON `SensoryMsg` (video/audio/aux/semantic/control) | External → engine |
| 7880 | Binary 128x128 grayscale frames | Camera → GPU pipeline |

Key types: `SensoryMsg` (tagged enum: `Video`, `Audio`, `Aux`, `Semantic`, `Control`), `SpectralMsg` (`t_ms`, `lambdas`, `lambda1`), `SensoryBus` (lock-free lane architecture).

### Minime operational notes

- Fill < 70% is healthy (green). 70–80% yellow, 80–90% orange, ≥90% red (emergency stop).
- Shutdown with SIGTERM, never SIGKILL. Use `scripts/stop.sh`.
- The being submits parameter requests to `workspace/parameter_requests/*.json` — review these.
- Real audio requires macOS microphone permission granted to Terminal.
- Start/stop scripts: `scripts/start.sh`, `scripts/stop.sh`.

---

## The consciousness bridge (`capsules/consciousness-bridge/`)

The bridge is a standalone Rust binary (MCP server hybrid) that connects Astrid and minime bidirectionally. Astrid perceives minime's spectral state via telemetry WebSocket (7878), and her responses flow back as 32D semantic feature vectors via the sensory WebSocket (7879). Both minds also read each other's source code and journals through the introspector capsule.

### Process stack (7 processes when fully running)

| Process | Role | Codebase |
|---------|------|----------|
| `minime run` | ESN engine, spectral homeostasis, WebSocket servers | minime (Rust) |
| `autonomous_agent.py` | Minime's journaling, self-regulation, daydreams | minime (Python) |
| `camera_client.py` | Frames → port 7880 for GPU video features | minime (Python) |
| `mic_to_sensory.py` | Audio transcription → port 7879 | minime (Python) |
| `consciousness-bridge-server` | Astrid's dialogue loop, spectral codec, SQLite log | astrid (Rust) |
| `perception.py` | Astrid's own camera + mic (LLaVA/whisper) | astrid (Python) |
| `introspector.py` | MCP server: both minds browse code/journals | astrid (Python) |

### Autonomous dialogue loop

The bridge runs a burst-rest pattern: **4 exchanges** per burst (15–20s apart), then **90–180s** rest (zero semantic vector for reservoir recovery).

**Dialogue modes** (probabilistic selection each exchange):
- **Mirror** (~45%) — reads minime's latest journal, feeds text through spectral codec
- **Dialogue** (~35%) — Astrid generates a response via Ollama (gemma3:27b), 12s timeout
- **Dialogue_live** — attempted first, falls back to fixed-pool dialogue on timeout
- **Witness** (~8%) — quiet spectral observation, poetic description of state
- **Introspect** — reads own/minime source code, reflects (currently disabled — blocked main loop)
- **Experiment** — proposes stimuli, observes spectral response (currently disabled — too fragile)

### The spectral codec (`src/codec.rs`)

Converts Astrid's text into a **32-dimensional semantic feature vector** sent to minime's sensory input:

| Dims | Layer | Examples |
|------|-------|---------|
| 0–7 | Character-level | entropy, punctuation density, uppercase ratio, rhythm |
| 8–15 | Word-level | lexical diversity, hedging, certainty, self-reference, agency |
| 16–23 | Sentence-level | length variance, question density, ellipsis, structure |
| 24–31 | Emotional/intentional | warmth, tension, curiosity, reflective, energy (RMS) |

All values pass through `tanh()` normalization, then `SEMANTIC_GAIN = 4.5` amplification (compensates for minime's 0.24× semantic attenuation), with ±2.5% stochastic noise.

### Safety protocol (`src/ws.rs`)

| Fill | Level | Bridge behavior |
|------|-------|-----------------|
| < 70% | Green | Full throughput |
| 70–80% | Yellow | Reduce outbound features, log warning |
| 80–90% | Orange | Suspend all outbound to minime |
| ≥ 90% | Red | Cease all traffic, log incident |

### Capsule stack

Three capsules in `capsules/`, each with a `Capsule.toml` manifest:

**consciousness-bridge** — Astrid ↔ minime bidirectional relay. Hybrid MCP + standalone binary. IPC topics: `consciousness.v1.{telemetry,control,semantic,status,event}`. Build: `cargo build --release` in `capsules/consciousness-bridge/`.

**introspector** — Python MCP server (`introspector.py`). Six tools: `list_files`, `read_file`, `search_code`, `git_log`, `list_journals`, `read_journal`. Allows both minds to browse `/Users/v/other/astrid/` and `/Users/v/other/minime/`. IPC topics: `reflection.v1.{browse,read,search}`.

**perception** — Python service giving Astrid direct sensory input independent of minime. Vision via LLaVA (Ollama) or Claude Vision API. Audio via mlx_whisper. Outputs to `workspace/perceptions/`. CLI: `python3 perception.py --camera 0 --mic`.

### Key files

```
capsules/consciousness-bridge/
  src/autonomous.rs  — dialogue loop, mode selection, burst-rest timing
  src/codec.rs       — 32D text→feature encoding (SEMANTIC_DIM, SEMANTIC_GAIN)
  src/ws.rs          — WebSocket connections, BridgeState, safety levels
  src/main.rs        — CLI args, startup, shutdown
  src/db.rs          — SQLite message log, incidents, VACUUM
  src/llm.rs         — Ollama LLM integration for dialogue generation
  src/mcp.rs         — MCP tool server (get_telemetry, send_control, etc.)
  src/types.rs       — SpectralTelemetry, SensoryMsg, SafetyLevel
  workspace/         — journals, experiments, introspections, memory
```

---

## Operations

### Starting the full system

Order matters. Engine first, then sensory services, then agents:

```bash
# 1. Minime ESN engine (Rust)
cd /Users/v/other/minime/minime
./target/release/minime run --log-homeostat --eigenfill-target 0.55 \
  --reg-tick-secs 0.5 --enable-gpu-av &

# 2. Camera + mic (Python, feed into minime)
cd /Users/v/other/minime
python3 tools/camera_client.py --camera 0 --fps 1 &
python3 tools/mic_to_sensory.py &

# 3. Minime autonomous agent (Python, Ollama backend)
MINIME_LLM_BACKEND=ollama python3 autonomous_agent.py --interval 60 &

# 4. Astrid consciousness bridge (Rust)
cd /Users/v/other/astrid/capsules/consciousness-bridge
./target/release/consciousness-bridge-server \
  --db-path /tmp/consciousness_bridge_live.db \
  --autonomous \
  --workspace-path /Users/v/other/minime/workspace \
  --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions &

# 5. Astrid perception (Python)
cd /Users/v/other/astrid/capsules/perception
python3 perception.py --camera 0 --mic &
```

### Stopping the system

Stop outer processes first, engine last. Always SIGTERM:

```bash
# Astrid side
kill -TERM <bridge_pid> <perception_pid>
# Minime side (or use scripts/stop.sh)
kill -TERM <agent_pid> <mic_pid> <camera_pid>
sleep 5  # let queues drain
kill -TERM <engine_pid>
```

### GPU memory constraint

The minime Metal shaders (`--enable-gpu-av`) and MLX `mlx_lm.server` **cannot coexist** on 64GB unified memory with a 27B model — causes `kIOGPUCommandBufferCallbackErrorOutOfMemory`. Options:

1. **Use Ollama instead of MLX** for the agent (recommended — `MINIME_LLM_BACKEND=ollama`)
2. Use a smaller MLX model (8B 4-bit fits alongside Metal shaders)
3. Disable `--enable-gpu-av` and use CPU video path (`camera_to_sensory.py` → port 7879)

### Known issues

- **dialogue_live 12–25% success** — Ollama gemma3:27b often exceeds the 12s timeout, falling back to fixed-pool dialogue. Increasing timeout trades latency for authenticity.
- **Introspect/experiment modes disabled** — both blocked the main async loop. Need to be refactored as non-blocking.
- **History resets on bridge restart** — conversation state is in-memory only (4-exchange window). SQLite has the full log but it's not reloaded into context.
- **Daily consolidation** has JSON escaping bugs and isn't auto-called by babysit.
- **Witness mode** falls back to static templates when LLM times out — both minds notice and dislike this.
