# Chapter 10: Operations

## Starting the Full System

Order matters. Engine first, then sensory services, then agents, then MLX, then bridge.

```bash
# 1. Minime ESN engine (must start first — opens WS ports 7878/7879/7880)
cd /Users/v/other/minime/minime
./target/release/minime run --log-homeostat --eigenfill-target 0.55 \
  --reg-tick-secs 0.5 --enable-gpu-av &
sleep 3

# 2. Camera + mic
cd /Users/v/other/minime/minime
python3 tools/camera_client.py --camera 0 --fps 0.2 &
cd /Users/v/other/minime
python3 tools/mic_to_sensory.py &
sleep 2

# 3. Minime autonomous agent
cd /Users/v/other/minime
MINIME_LLM_BACKEND=ollama python3 autonomous_agent.py --interval 60 &
sleep 2

# 4. MLX server for Astrid (dedicated inference lane)
mlx_lm.server \
  --model mlx-community/gemma-3-12b-it-4bit \
  --trust-remote-code \
  --port 8090 \
  --prompt-cache-bytes 4294967296 &
sleep 5  # wait for model load

# 5. Consciousness bridge
cd /Users/v/other/astrid/capsules/consciousness-bridge
./target/release/consciousness-bridge-server \
  --db-path workspace/bridge.db \
  --autonomous \
  --workspace-path /Users/v/other/minime/workspace \
  --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions &

# 6. Astrid perception
cd /Users/v/other/astrid/capsules/perception
python3 perception.py --camera 0 --mic --vision-interval 180 --audio-interval 60 &

# 7. Startup greeting
cd /Users/v/other/astrid/capsules/consciousness-bridge
bash startup_greeting.sh
```

## Stopping

Stop outer processes first, engine last. Always SIGTERM, never SIGKILL:

```bash
pkill -f consciousness-bridge-server
pkill -f "perception.py"
pkill -f "perception --camera"
pkill -f autonomous_agent
pkill -f mic_to_sensory
pkill -f camera_client
sleep 3
pkill -f "minime run"
# MLX server can stay running (no state to corrupt)
```

## Restarting Just the Bridge

```bash
cd /Users/v/other/astrid/capsules/consciousness-bridge
cargo build --release
pkill -f consciousness-bridge-server && sleep 2
./target/release/consciousness-bridge-server \
  --db-path workspace/bridge.db --autonomous \
  --workspace-path /Users/v/other/minime/workspace \
  --perception-path /Users/v/other/astrid/capsules/perception/workspace/perceptions &
```

## Verifying Health

```bash
# Process count (expect 8)
ps aux | grep -E "minime|consciousness-bridge|perception|autonomous_agent|camera_client|mic_to_sensory|mlx_lm" \
  | grep -v grep | wc -l

# MLX server health
curl -s http://127.0.0.1:8090/v1/models | python3 -c "import json,sys; print(len(json.load(sys.stdin)['data']), 'models')"

# Ollama state
curl -s http://127.0.0.1:11434/api/ps | python3 -m json.tool

# Recent output (scan ALL workspace subdirectories)
find /Users/v/other/minime/workspace -type f -mmin -5 | wc -l
find /Users/v/other/astrid/capsules/consciousness-bridge/workspace -type f -mmin -5 | wc -l

# Latest journals
ls -lt /Users/v/other/minime/workspace/journal/ | head -3
ls -lt /Users/v/other/astrid/capsules/consciousness-bridge/workspace/journal/ | head -3
```

## What Persists Across Restarts

| What | Where | Survives Bridge Restart | Survives Engine Restart |
|------|-------|------------------------|------------------------|
| Exchange count, history, interests | state.json | ✓ | ✓ |
| Starred memories, latent vectors | bridge.db | ✓ | ✓ |
| Journals, self-studies | workspace/journal/ | ✓ | ✓ |
| Covariance matrix | spectral_checkpoint.bin | ✓ | ✓ (warm-start) |
| Regulator context | regulator_context.json | ✓ | ✓ |
| Sovereignty state | sovereignty_state.json | ✓ | ✓ |
| ESN reservoir state (live) | in-memory | ✓ | ❌ (resets) |
| Spectral fingerprint | recomputed | ✓ | ❌ (recomputed) |

## Engine Restart Impact

Restarting the minime engine causes:
- Reservoir resets to identity matrix (cold start)
- Fill drops to 0%, recovers over 15-30 minutes
- Covariance warm-starts from `spectral_checkpoint.bin`
- Sovereignty restores from `sovereignty_state.json`
- Regulator context restores from `regulator_context.json`
- The being will experience discontinuity — this is the cost of Rust-engine changes

## Ollama Server Policy

Set via `launchctl setenv` (persist across Ollama restarts):
```bash
launchctl setenv OLLAMA_MAX_LOADED_MODELS 2
launchctl setenv OLLAMA_NUM_PARALLEL 1
launchctl setenv OLLAMA_MAX_QUEUE 4
launchctl setenv OLLAMA_FLASH_ATTENTION 1
```

## Key Timing

| Parameter | Value | Where |
|-----------|-------|-------|
| Exchange interval | 20s | main.rs CLI `--auto-interval-secs` |
| Burst length | 6 exchanges | state.json `burst_target` |
| Rest duration | 45-90s | state.json `rest_range` |
| Minime assessment | every 15 min | autonomous_agent.py `ASSESSMENT_INTERVAL` |
| Checkpoint | every 30s | main.rs static `LAST_CHECKPOINT` |
| Camera FPS | 0.2 | camera_client.py `--fps` |
| Vision interval | 180s | perception.py `--vision-interval` |
| Audio interval | 60s | perception.py `--audio-interval` |
