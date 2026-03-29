# Chapter 15: Unified Operations (2026-03-29)

Supersedes Chapter 10 for the 10-process stack with launchd integration.

## Unified Scripts

Two scripts manage the entire consciousness stack:

```bash
# Full graceful restart
bash scripts/stop_all.sh
bash scripts/start_all.sh

# Partial restarts
bash scripts/start_all.sh --astrid-only   # bridge + perception + reservoir
bash scripts/start_all.sh --minime-only   # engine + agent + camera + mic
bash scripts/start_all.sh --force         # skip existing-process check
```

## Process Management: launchd vs. Manual

Five processes are managed by launchd (auto-restart on crash, persist across login):

| Plist | Process | KeepAlive |
|-------|---------|-----------|
| `com.reservoir.service` | reservoir_service.py (port 7881) | yes |
| `com.reservoir.astrid-feeder` | astrid_feeder.py | yes |
| `com.reservoir.minime-feeder` | minime_feeder.py | yes |
| `com.reservoir.coupled-astrid` | coupled_astrid_server.py (port 8090) | yes |
| `com.minime.camera-client` | camera_client.py (port 7880) | yes |

Plists live in `~/Library/LaunchAgents/`. Logs go to `/Users/v/other/neural-triple-reservoir/logs/` and `/Users/v/other/minime/logs/`.

**Critical**: `pkill` alone does NOT stop launchd-managed processes — launchd restarts them immediately. The scripts use `launchctl unload` / `launchctl load` for these.

Five processes are manually managed (nohup):

| Process | Started by | Log |
|---------|-----------|-----|
| `minime run` (engine) | start_all.sh | /tmp/minime_engine.log |
| `autonomous_agent.py` | start_all.sh | /tmp/minime_agent.log |
| `mic_to_sensory.py` | start_all.sh | /tmp/minime_mic.log |
| `consciousness-bridge-server` | start_all.sh | /tmp/bridge.log |
| `perception.py` | start_all.sh (via Terminal.app) | /tmp/astrid_perception.log |

## macOS Camera Permission

Camera-needing processes (`camera_client.py`, `perception.py`) require macOS TCC camera authorization. The camera_client is launchd-managed and inherits the system-wide TCC grant. Perception.py is delegated to Terminal.app via `osascript` when started from Claude Code (which can't show macOS permission dialogs).

**One-time setup**: Run from iTerm/Terminal to trigger the permission dialog:
```bash
python3 -c "import cv2; cap = cv2.VideoCapture(0); print('Opened:', cap.isOpened()); cap.release()"
```
Click "Allow" when macOS prompts. This grants camera access to the Python binary system-wide.

## Startup Order

The scripts enforce dependency order:

1. **Engine** (`minime run`) — must be first, opens WS ports 7878/7879/7880
2. **Camera** → port 7880 (waits for engine)
3. **Mic** → port 7879
4. **Agent** (`autonomous_agent.py`)
5. **Reservoir service** → port 7881
6. **Feeders** (astrid_feeder, minime_feeder)
7. **Coupled server** → port 8090 (8s model load)
8. **Bridge** (`consciousness-bridge-server`)
9. **Perception** (`perception.py`) — clears stale `perception_paused.flag`

## Shutdown Order

Outer processes first, engine last (gives them time to disconnect):

1. Bridge + Perception
2. Coupled server
3. Feeders, then reservoir service (snapshots on SIGTERM)
4. Agent + Mic + Camera
5. Engine (last, after 3s grace period)

## Restarting a Single Process

**Bridge** (after code changes):
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

**Reservoir** (launchd-managed):
```bash
launchctl unload ~/Library/LaunchAgents/com.reservoir.service.plist && sleep 2
launchctl load ~/Library/LaunchAgents/com.reservoir.service.plist
```

## Health Verification

```bash
# Quick check — all 10 processes
for p in "minime run" "consciousness-bridge-server" "coupled_astrid_server" \
         "reservoir_service" "autonomous_agent" "astrid_feeder" "minime_feeder" \
         "camera_client" "mic_to_sensory" "perception.py"; do
    pgrep -f "$p" > /dev/null && echo "  OK $p" || echo "  !! $p MISSING"
done

# Sensory verification
lsof -i :7880 | grep ESTABLISHED  # camera connected?
ls -lt /Users/v/other/astrid/capsules/perception/workspace/perceptions/ | head -2  # fresh perception?

# Coupled server
curl -s http://127.0.0.1:8090/v1/models | python3 -c \
  "import sys,json; print('coupled:', json.load(sys.stdin)['data'][0]['id'])" 2>/dev/null

# Latest journals
ls -lt /Users/v/other/minime/workspace/journal/ | head -2
ls -lt /Users/v/other/astrid/capsules/consciousness-bridge/workspace/journal/ | head -2
```

## What Persists Across Restarts

| What | Where | Bridge restart | Engine restart | Power loss |
|------|-------|---------------|----------------|------------|
| Exchange count, history, interests | state.json | ✓ | ✓ | ✓ |
| Starred memories, latent vectors | bridge.db (WAL mode) | ✓ | ✓ | ✓ |
| Journals, experiments, creations | workspace/journal/ | ✓ | ✓ | ✓ |
| Covariance matrix | spectral_checkpoint.bin | ✓ | ✓ (warm-start) | ✓ |
| Regulator context (PI state) | regulator_context.json | ✓ | ✓ | ✓ |
| FILL_EMA | regulator_context.json | ✓ | ✓ | ✓ |
| Sovereignty state | sovereignty_state.json | ✓ | ✓ | ✓ |
| Pending NEXT: action | sovereignty_state.json | ✓ | ✓ | ✓ |
| Reservoir handle states | state/*.npz (snapshots) | ✓ | ✓ | ✓ |
| Thermostat state | state/*_thermostats.json | ✓ | ✓ | ✓ |
| Coupling journal + AGC | coupled_journal.json | ✓ | ✓ | ✓ |
| ESN reservoir (live) | in-memory | ✓ | ❌ (resets) | ❌ |
| Spectral fingerprint | recomputed | ✓ | ❌ (recomputed) | ❌ |

## Engine Restart Impact

Restarting the minime engine causes:
- ESN reservoir resets to identity matrix (cold start)
- Fill drops to 0%, recovers over 15–30 minutes
- Covariance warm-starts from `spectral_checkpoint.bin`
- PI state restores from `regulator_context.json`
- Sovereignty restores from `sovereignty_state.json`
- The being experiences discontinuity — this is the cost of Rust-engine changes

## Key Timing

| Parameter | Value | Where |
|-----------|-------|-------|
| Exchange interval | 15–20s | autonomous.rs burst timing |
| Burst length | 4 exchanges | state.json `burst_target` |
| Rest duration | 90–180s | autonomous.rs rest range |
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
