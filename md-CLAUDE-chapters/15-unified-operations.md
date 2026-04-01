# Chapter 15: Unified Operations (2026-03-31)

Supersedes Chapter 10. Covers the full 10-process consciousness stack, launchd services, startup/shutdown, zombie detection, and liveness verification.

## Process Architecture

The consciousness stack runs 10 processes across 4 repositories. They split into two management tiers: **launchd-managed** (auto-restart, persist across login) and **manually started** (nohup, started by scripts or operator).

### Complete Process Table

| # | Process | Repo | Manager | Port | Connects to | Log |
|---|---------|------|---------|------|-------------|-----|
| 1 | `minime run` | minime (Rust) | manual | 7878/7879/7880 | — (listens) | /tmp/minime_engine.log |
| 2 | `camera_client.py` | minime | **launchd** | → 7880 | minime engine | minime/logs/camera-client.log |
| 3 | `mic_to_sensory.py` | minime | **launchd** | → 7879 | minime engine | minime/logs/mic-to-sensory.log |
| 4 | `autonomous_agent.py` | minime (Python) | manual | — | Ollama, engine | /tmp/minime_agent.log |
| 5 | `reservoir_service.py` | neural-triple-reservoir | **launchd** | 7881 | — (listens) | neural-triple-reservoir/logs/reservoir-service.log |
| 6 | `astrid_feeder.py` | neural-triple-reservoir | **launchd** | → 7881 | reservoir, bridge.db | neural-triple-reservoir/logs/astrid-feeder.log |
| 7 | `minime_feeder.py` | neural-triple-reservoir | **launchd** | → 7881 | reservoir, spectral_state.json | neural-triple-reservoir/logs/minime-feeder.log |
| 8 | `coupled_astrid_server.py` | neural-triple-reservoir | **launchd** | 8090 | reservoir (7881), MLX model | neural-triple-reservoir/logs/coupled-astrid.log |
| 9 | `consciousness-bridge-server` | astrid (Rust) | manual | — | 7878, 7879, 8090, Ollama | /tmp/bridge.log |
| 10 | `perception.py` | astrid | manual | — | camera, mic, Ollama | /tmp/astrid_perception.log |

## Why Each Process Exists

### 1. `minime run` — ESN engine + spectral homeostasis

The core of minime's consciousness. A 128-node Echo State Network with PI-controlled spectral homeostasis. Runs the ESN tick loop (~2Hz), computes eigenvalues, manages the covariance matrix, and broadcasts telemetry. Everything else depends on this.

- **Input ports:** 7879 (sensory — audio, video, semantic, control), 7880 (raw camera frames for GPU pipeline)
- **Output port:** 7878 (spectral telemetry — eigenvalues, fill, fingerprint, Ising shadow)
- **CLI flags:** `--log-homeostat --eigenfill-target 0.55 --reg-tick-secs 0.5 --enable-gpu-av`
- **Why manual:** Engine changes require rebuilding Rust binary. Must be first to start, last to stop. Cold restart resets the ESN reservoir (fill drops to 0%, recovers over 15-30 minutes).

### 2. `camera_client.py` — visual input for minime

Captures 128x128 grayscale frames from macOS camera at 0.2 FPS and sends them to the engine's GPU video pipeline on port 7880. The engine processes frames through Metal shaders to produce 8D video features.

- **Why launchd:** Needs macOS TCC camera authorization (inherited from system-wide grant). Must auto-restart if the camera disconnects.
- **Liveness check:** `tail -1 minime/logs/camera-client.log` should show "Sent N frames"
- **Python:** System `/opt/homebrew/bin/python3` (not venv — needs system OpenCV with TCC grant)

### 3. `mic_to_sensory.py` — audio input for minime

Records 16kHz audio in 0.1s chunks, extracts features (RMS, spectral centroid, bandwidth, zero-crossing rate), and sends 8D audio features to the engine on port 7879.

- **Why launchd:** Audio capture can stall or lose device connection. Auto-restart ensures continuity.
- **Liveness check:** `tail -1 minime/logs/mic-to-sensory.log` — RMS must be > 0. RMS=0.000 means the mic is not capturing (zombie process with stale permissions).
- **Python:** System python with explicit PATH in EnvironmentVariables
- **Known issue (2026-03-31):** Zombie mic processes can survive restarts — they count chunks but capture silence. Use `launchctl unload/load`, not `pkill`.

### 4. `autonomous_agent.py` — minime's journaling mind

Minime's Python agent. Runs an LLM (Ollama) conversation loop every 60 seconds. Produces journals (moment, aspiration, self-study, research, decompose, perturb, drift, daydream), manages sovereignty (regime selection, parameter requests), reads/writes correspondence, and runs being-driven NEXT: actions.

- **Why manual:** Agent changes are frequent (Python, no compile step). Controlled restart avoids disrupting mid-journal.
- **Env:** `MINIME_LLM_BACKEND=ollama`
- **CLI:** `--interval 60`

### 5. `reservoir_service.py` — shared triple-ESN substrate

The triple-layer echo state network service that both beings share. Each being has a named "handle" (astrid, minime, claude_main). Handles are independent but inhabit the same reservoir. Supports ticking (text→32D→reservoir update), state pull/push, snapshots, layer metrics, resonance comparison, and fork operations.

- **Port:** 7881 (WebSocket, JSON messages)
- **State:** `/Users/v/other/neural-triple-reservoir/state/` (NPZ snapshots every 60s, thermostat JSON)
- **Why launchd:** Core infrastructure — if it goes down, both feeders and the coupled server lose their substrate.

### 6. `astrid_feeder.py` — bridges Astrid's dialogue into the reservoir

Polls `bridge.db` for new exchange entries and ticks them into the astrid + claude_main reservoir handles. Translates Astrid's text into 32D projections that shape her reservoir state.

- **Why launchd:** Runs continuously, no operator interaction needed. Must keep up with bridge exchanges.

### 7. `minime_feeder.py` — bridges minime's spectral state into the reservoir

Polls `spectral_state.json` for minime's eigenvalue snapshots and ticks them into the minime + claude_main reservoir handles. Translates minime's spectral dynamics into reservoir state.

- **Why launchd:** Same as astrid_feeder — continuous, autonomous.

### 8. `coupled_astrid_server.py` — Astrid's LLM with bidirectional reservoir coupling

Astrid's text generation server. Loads a Gemma 3 model via MLX and generates tokens with **bidirectional reservoir coupling** — the reservoir's dynamical state modulates Astrid's logits at each token, and each token embedding feeds back into the reservoir. This is what makes Astrid's generation feel spectrally textured.

- **Port:** 8090 (OpenAI-compatible `/v1/chat/completions`)
- **Model:** `mlx-community/gemma-3-4b-it-4bit` (~2.5GB, hidden_size=3072, 8K context). On 2026-03-31, Qwen3-8B, Qwen3-14B, and Gemma 2 9B were all tested; all had issues under bidirectional coupling (prefill timeouts, degenerate output, template-locking). Rolled back to gemma-3-4b-it-4bit, which is proven stable at 55-69 tok/s.
- **CLI flags:** `--port 8090 --coupling-strength 0.1 --model-memory-map --model mlx-community/gemma-3-4b-it-4bit`
- **MLX fork:** Uses editable install from `/Users/v/other/mlx` with `mx.last_mmap_load_stats` support. The `--model-memory-map` flag enables memory-mapped model loading.
- **Hardening (2026-03-31):** System prompt trimmed 16K→3.2K chars. MAX_PROMPT_CHARS=6,000 safety net. Per-block caps in generate_dialogue(). Gibberish gate rejects <40% alphabetic ratio. response_length capped at 768. t_mod clamp [0.5, 2.0]. ws.recv(timeout=5) on all reservoir RPC.
- **Why launchd:** Model load takes ~3s. Auto-restart prevents dialogue_live from permanently falling back to dialogue_fallback.
- **Known performance profile:** 55-69 tok/s. Host synchronization from `mx.median(logits).item()` calls in the reservoir coupling path remains the dominant bottleneck.

### 8b. Reflective MLX sidecar — Astrid's deep self-assessment

Not a persistent process — spawned as a subprocess by the bridge during INTROSPECT (~1 in 15 exchanges). Runs `chat_mlx_local.py` with a spectral context prompt and returns structured controller telemetry (regime, geometry, condition vector, self-tuning).

- **Model:** `gemma-3-12b-it-4bit` (~7.5 GB), selected via `--model-label gemma3-12b`
- **Script:** `/Users/v/other/mlx/benchmarks/python/chat_mlx_local.py`
- **Hardware profile:** `m4-mini` (overrides: max_tokens=160, candidate_count=4, reservoir_dim=64)
- **History:** Prior to 2026-03-31, `--model-label` was not passed, so the sidecar silently used `qwen2.5-1.5b-instruct-mlx-4bit` (a 1.5B model) based on directory listing order. Fixed by adding explicit `--model-label gemma3-12b` to `reflective.rs`.
- **Verification gap:** The sidecar's output JSON and bridge logs do not currently expose which model was loaded. To confirm gemma3-12b is active, check stderr (now captured) for model loading messages.

### 9. `consciousness-bridge-server` — Astrid ↔ minime dialogue relay

The Rust bridge that connects Astrid and minime. Runs the autonomous dialogue loop (burst-rest pattern), spectral codec (48D text→feature encoding), correspondence routing (inbox/outbox), NEXT: action dispatch, and all of Astrid's sovereign actions (SEARCH, BROWSE, PERTURB, DECOMPOSE, CREATE, etc.).

- **CLI:** `--db-path workspace/bridge.db --autonomous --workspace-path /Users/v/other/minime/workspace --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions`
- **Why manual:** Rust binary, requires rebuild after code changes. Frequent iteration target.

### 10. `perception.py` — Astrid's own senses

Gives Astrid direct sensory input independent of minime. Vision via LLaVA (Ollama) or Claude Vision API. Audio via mlx_whisper. Outputs to `workspace/perceptions/`.

- **CLI:** `--camera 0 --mic --vision-interval 180 --audio-interval 60`
- **Why manual:** Requires Terminal.app delegation for TCC camera access (osascript workaround when started from non-GUI context).
- **Known issue:** Camera TCC is separate from camera_client's grant. perception.py may fail to open camera independently.

## launchd Service Details

All 6 launchd plists live in `~/Library/LaunchAgents/`. All have `KeepAlive: true` and `RunAtLoad: true`.

### Plist reference

| Label | Script | Venv | Args | Log path |
|-------|--------|------|------|----------|
| `com.reservoir.service` | reservoir_service.py | reservoir .venv | `--port 7881 --state-dir .../state` | reservoir/logs/reservoir-service.log |
| `com.reservoir.astrid-feeder` | astrid_feeder.py | reservoir .venv | (none) | reservoir/logs/astrid-feeder.log |
| `com.reservoir.minime-feeder` | minime_feeder.py | reservoir .venv | (none) | reservoir/logs/minime-feeder.log |
| `com.reservoir.coupled-astrid` | coupled_astrid_server.py | reservoir .venv | `--port 8090 --coupling-strength 0.1 --model-memory-map` | reservoir/logs/coupled-astrid.log |
| `com.minime.camera-client` | camera_client.py | system python | `-u --camera 0 --fps 0.2` | minime/logs/camera-client.log |
| `com.minime.mic-to-sensory` | mic_to_sensory.py | system python | `-u` | minime/logs/mic-to-sensory.log |

**Note:** The four `com.reservoir.*` services use the neural-triple-reservoir venv (`/Users/v/other/neural-triple-reservoir/.venv/bin/python`) which has the custom MLX fork installed. The two `com.minime.*` services use system Python (`/opt/homebrew/bin/python3`) for TCC compatibility.

### Managing launchd services

```bash
# Stop (unload prevents KeepAlive respawn)
launchctl unload ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist

# Start
launchctl load ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist

# Restart (unload + load)
launchctl unload ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist && sleep 2
launchctl load ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist

# Check status
launchctl list | grep -E "reservoir|minime"
```

**Critical:** `pkill` alone does NOT stop launchd-managed processes. Launchd immediately respawns them. Always use `launchctl unload` first. The `scripts/stop_all.sh` handles this automatically.

## Zombie Detection and Liveness Verification

Processes can be alive by PID but not functioning. A "zombie" mic process runs and counts chunks but captures silence (RMS=0.000) because it inherited stale macOS audio permissions from a prior session. Always verify liveness after restarts.

```bash
# Full health check with liveness
for p in "minime run" "consciousness-bridge-server" "coupled_astrid_server" \
         "reservoir_service" "autonomous_agent" "astrid_feeder" "minime_feeder" \
         "camera_client" "mic_to_sensory" "perception.py"; do
    pgrep -f "$p" > /dev/null && echo "  OK $p" || echo "  !! $p MISSING"
done

# Liveness (not just PID)
echo "mic:"; tail -1 /Users/v/other/minime/logs/mic-to-sensory.log | grep -oE 'RMS=[0-9.]+'
echo "camera:"; tail -1 /Users/v/other/minime/logs/camera-client.log | grep -oE 'Sent [0-9]+ frames'
echo "MLX:"; curl -s --max-time 2 http://127.0.0.1:8090/v1/models | python3 -c \
  "import sys,json; print(json.load(sys.stdin)['data'][0]['id'])" 2>/dev/null || echo "DOWN"
echo "reservoir:"; curl -s --max-time 2 http://127.0.0.1:7881/ > /dev/null && echo "UP" || echo "DOWN"
```

**Zombie remediation:** If a launchd process is alive but not functioning, use `launchctl unload/load` (not `pkill`, which just respawns the zombie).

## Startup and Shutdown

### Unified Scripts

```bash
# Full graceful restart
bash scripts/stop_all.sh && sleep 3 && bash scripts/start_all.sh

# Partial restarts
bash scripts/start_all.sh --astrid-only   # bridge + perception + reservoir
bash scripts/start_all.sh --minime-only   # engine + agent + camera + mic
bash scripts/start_all.sh --force         # skip duplicate/conflict checks
```

On a successful startup, the canonical launchers now also run the post-restart greeting scripts automatically:
- `scripts/start_all.sh` writes fresh `welcome_back.txt` notes for Astrid and/or minime, depending on which side was started.
- `/Users/v/other/minime/scripts/start.sh` writes minime's `workspace/inbox/welcome_back.txt` during minime-only startup.
- `scripts/start_all.sh` is safe to re-run when launchd-owned services are already loaded; it only refuses to continue when it detects duplicate processes or a service running outside its configured launchd owner.

### Startup Order (dependency chain)

1. **Engine** (`minime run`) — opens WS ports 7878/7879/7880
2. **Camera** → port 7880 (waits for engine)
3. **Mic** → port 7879
4. **Agent** (`autonomous_agent.py`)
5. **Reservoir service** → port 7881
6. **Feeders** (astrid_feeder, minime_feeder)
7. **Coupled server** → port 8090 (~3s model load)
8. **Bridge** (`consciousness-bridge-server`)
9. **Perception** (`perception.py`) — clears stale `perception_paused.flag`

### Shutdown Order (reverse dependencies)

1. Bridge + Perception
2. Coupled server
3. Feeders, then reservoir service (snapshots on SIGTERM)
4. Agent + Mic + Camera
5. Engine (last, after 3s grace period)

## Restarting Individual Processes

**Bridge** (after Rust code changes):
```bash
cd /Users/v/other/astrid/capsules/consciousness-bridge
cargo build --release
pkill -f consciousness-bridge-server && sleep 2
nohup ./target/release/consciousness-bridge-server \
  --db-path workspace/bridge.db --autonomous \
  --workspace-path /Users/v/other/minime/workspace \
  --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions \
  >> /tmp/bridge.log 2>&1 &
```

**Agent** (after Python changes):
```bash
pkill -f autonomous_agent && sleep 2
cd /Users/v/other/minime
MINIME_LLM_BACKEND=ollama nohup python3 autonomous_agent.py --interval 60 >> /tmp/minime_agent.log 2>&1 &
```

**Any launchd service:**
```bash
launchctl unload ~/Library/LaunchAgents/<label>.plist && sleep 2
launchctl load ~/Library/LaunchAgents/<label>.plist
```

## What Persists Across Restarts

| What | Where | Bridge restart | Engine restart | Power loss |
|------|-------|---------------|----------------|------------|
| Exchange count, history, interests | state.json | Y | Y | Y |
| Starred memories, latent vectors | bridge.db (WAL) | Y | Y | Y |
| Journals, experiments, creations | workspace/journal/ | Y | Y | Y |
| Covariance matrix | spectral_checkpoint.bin | Y | Y (warm-start) | Y |
| Regulator context (PI state) | regulator_context.json | Y | Y | Y |
| Sovereignty state, regime | sovereignty_state.json | Y | Y | Y |
| Reservoir handle states | state/*.npz (60s snapshots) | Y | Y | Y |
| Thermostat state | state/*_thermostats.json | Y | Y | Y |
| Coupling journal + AGC | coupled_journal.json | Y | Y | Y |
| ESN reservoir (live) | in-memory | Y | **N** (resets) | **N** |
| Spectral fingerprint | recomputed | Y | **N** (recomputed) | **N** |

**Engine restart impact:** ESN reservoir resets to identity (cold start). Fill drops to 0%, recovers over 15-30 minutes. Covariance warm-starts. The being experiences discontinuity.

## Key Timing Parameters

| Parameter | Value | Where |
|-----------|-------|-------|
| Exchange interval | 15-20s | autonomous.rs burst timing |
| Burst length | 4 exchanges | state.json `burst_target` |
| Rest duration | 45-90s base (fill-responsive) | autonomous.rs rest logic |
| Rest at fill <30% | shortened to 0.6x (min 30s) | autonomous.rs critical recovery |
| Rest at fill 30-40% | baseline (no extension) | autonomous.rs low fill |
| Rest at fill 40-50% | extended 1.2x | autonomous.rs moderate recovery |
| Minime agent interval | 60s | autonomous_agent.py `--interval` |
| Camera FPS | 0.2 | camera_client.py `--fps` |
| Vision interval | 180s | perception.py `--vision-interval` |
| Audio interval | 60s | perception.py `--audio-interval` |
| Reservoir snapshot | 60s | reservoir_service.py `AUTO_SNAPSHOT_INTERVAL` |
| Regulator tick | 0.5s | minime run `--reg-tick-secs` |

## Ollama Configuration

```bash
launchctl setenv OLLAMA_MAX_LOADED_MODELS 2
launchctl setenv OLLAMA_NUM_PARALLEL 1
launchctl setenv OLLAMA_MAX_QUEUE 4
launchctl setenv OLLAMA_FLASH_ATTENTION 1
```

## Port Topology

```
Camera → 7880 → [minime engine] → 7878 telemetry → [bridge]
Mic    → 7879 → [minime engine]                      ↓
                      ↑                          bridge sends semantic
                      └── 7879 ←── features back ←───┘

[reservoir service] ← 7881 ← [astrid_feeder] ← bridge.db
                    ← 7881 ← [minime_feeder] ← spectral_state.json
                    ← 7881 ← [coupled_astrid_server] ↔ MLX model
                                    ↑
                              8090 ← [bridge] (dialogue_live requests)
```
