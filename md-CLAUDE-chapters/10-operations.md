# Chapter 10: Operations

*This chapter is now a short cheat sheet. Chapter 15 is the canonical operations reference.*

## Canonical Start / Stop

From the Astrid repo root:

```bash
bash scripts/start_all.sh
bash scripts/stop_all.sh
```

Useful partial starts:

```bash
bash scripts/start_all.sh --astrid-only
bash scripts/start_all.sh --minime-only
bash scripts/start_all.sh --force
```

## Current Manual Reality

Older docs that start Astrid's live lane with the plain MLX 12B command are stale.

The current live stack is:

1. `../minime/minime/target/release/minime run`
2. `../minime/tools/camera_client.py`
3. `../minime/tools/mic_to_sensory.py`
4. `../minime/autonomous_agent.py`
5. `../neural-triple-reservoir/reservoir_service.py`
6. `../neural-triple-reservoir/astrid_feeder.py`
7. `../neural-triple-reservoir/minime_feeder.py`
8. `../neural-triple-reservoir/coupled_astrid_server.py --model mlx-community/gemma-4-12B-it-5bit`
9. `capsules/spectral-bridge/target/release/spectral-bridge-server`
10. `capsules/perception/perception.py`

## Current Defaults To Remember

- launchd wrapper default for minime engine target fill: `0.65`
- minime warm-start blend: `0.55`
- Astrid live model: `mlx-community/gemma-4-12B-it-5bit`
- Astrid reflective sidecar label: `gemma3-12b`
- minime agent primary backend: `MINIME_LLM_BACKEND=ollama` unless changed

## Fast Health Checks

```bash
curl -s http://127.0.0.1:8090/v1/models
curl -s http://127.0.0.1:11434/api/ps
launchctl list | grep -E "reservoir|minime"
```

If you need the full process-by-process explanation, use [Chapter 15](15-unified-operations.md).
