# Chapter 17: Coupled Generation

*Ground truth as of March 31, 2026. Includes the 2026-03-31 model trial results and rollback to gemma-3-4b-it-4bit.*

How Astrid's text generation is bidirectionally coupled to the triple-layer reservoir. The reservoir shapes her words; her words shape the reservoir. This chapter covers the coupling architecture, model selection, prompt hardening, and the operational constraints that determine which models can actually serve this role.

**Codebase:** `/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py`, `mlx_reservoir.py`

## Architecture

```
                         ┌─────────────────────────────────┐
                         │     coupled_astrid_server.py     │
                         │          Port 8090               │
                         │                                  │
  Bridge request ──────► │  1. Format prompt (ChatML)       │
  (JSON, /v1/chat)       │  2. Pull h1/h2/h3 from service  │
                         │  3. Per-token loop:              │
                         │     ┌──────────────────────┐     │
                         │     │ token                 │     │
                         │     │   ↓                   │     │
                         │     │ embed_tokens(token)   │     │ ◄── LLM embedding table
                         │     │   ↓                   │     │
                         │     │ EmbeddingProjection   │     │ ◄── 3072 → 32D (frozen random)
                         │     │   ↓                   │     │
                         │     │ reservoir.step_multi  │     │ ◄── Triple ESN tick
                         │     │   ↓                   │     │
                         │     │ (y1, y2, y3)          │     │
                         │     │   ↓                   │     │
                         │     │ ReservoirLogitProc    │     │ ◄── Modulate next logits
                         │     │   ↓                   │     │
                         │     │ sample next token     │     │
                         │     └──────────────────────┘     │
                         │  4. Push evolved h1/h2/h3 back   │
                         │  5. Return response               │
                         └─────────────────────────────────┘
                                       ↕
                         ┌─────────────────────────────────┐
                         │     reservoir_service.py         │
                         │     Port 7881 (WebSocket)        │
                         │     192 nodes × 3 layers         │
                         │     Named handle: "astrid"       │
                         └─────────────────────────────────┘
```

The coupling is **per-token**: every single token Astrid generates feeds the reservoir, and the reservoir's output from that tick shapes the logit distribution for the *next* token. This is not post-hoc modulation — it is woven into the generation loop itself.

## Three-Timescale Logit Modulation

The `ReservoirLogitProcessor` applies reservoir influence at three timescales, one per ESN layer:

| Layer | Output | Timescale | What it modulates | How |
|-------|--------|-----------|-------------------|-----|
| h1 | y1 | Fast (token-level) | Temperature / confidence | Positive y1 → lower temp (more confident); negative → higher (exploratory). Applied as `logits * (1/t_mod)` |
| h2 | y2 | Medium (phrase-level) | Repetition / diversity | Positive y2 → boost entropy (more diverse); negative → allow repetition. Applied as uniform entropy nudge |
| h3 | y3 | Slow (discourse-level) | Top-p / tonal drift | Positive y3 → narrow nucleus (precise); negative → wide (exploratory). Applied as below-median tail scaling |

All modulations are gated by `coupling_strength` (default 0.10, range 0.02–0.30). At 0.10, each layer shifts its target by at most ±10%.

**File:** `/Users/v/other/neural-triple-reservoir/mlx_reservoir.py` lines 190–272

## Embedding Projection

Each token's embedding (from the LLM's `embed_tokens` layer) is projected from the model's hidden dimension to the reservoir's 32D input space:

```python
# EmbeddingProjection: frozen random projection, seed=137
W = normal(0, 1/sqrt(embed_dim), shape=(embed_dim, 32))
output = tanh(embedding @ W)  # bounded [-1, 1]
```

This projection is deterministic (seeded) but the seed is fixed, so changing models changes the projection matrix shape and values. **When switching models, the coupling journal must be cleared** — the AGC history is calibrated to the old projection.

| Model | embed_dim | Projection shape | Notes |
|-------|-----------|-----------------|-------|
| gemma-3-4b-it-4bit | 3072 | 3072 × 32 | **Current** (2026-03-27, restored 2026-03-31) |
| Qwen3-8B-4bit | 4096 | 4096 × 32 | Tested 2026-03-31, unstable under coupling |
| Qwen3-14B-4bit | 5120 | 5120 × 32 | Tested 2026-03-31, prefill too slow |
| Gemma 2 9B | 3584 | 3584 × 32 | Tested 2026-03-31, degenerate output under coupling |

## Adaptive Gain Control (AGC)

The coupling strength auto-adjusts based on y-value variance over a 30-generation sliding window:

- **Variance < 0.01:** Reservoir is quiet → multiply strength by 1.05 (amplify so it has more voice)
- **Variance > 1.0:** Reservoir is dominating → multiply strength by 0.95 (attenuate so the LLM retains agency)
- **Bounds:** Hard-clamped to [0.02, 0.30]
- **Rate:** 5% nudge per generation → ~20 generations to double or halve

The AGC journal persists to `state/coupling_journal.json` (50-entry rolling window) so coupling calibration survives restarts.

**File:** `/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py` lines 659–695

## Model-Agnostic Design

The coupled server auto-detects model architecture at load time:

**Hidden size detection:**
1. `model.args.hidden_size` — Llama, Mistral, Qwen families
2. `model.args.text_config["hidden_size"]` — Gemma-3 family

**Embedding table location:**
1. `model.language_model.model.embed_tokens` — Gemma-3
2. `model.model.embed_tokens` — Llama, Mistral, Qwen

**Stop tokens:** Resolved once at init by encoding special strings (`<|im_end|>`, `<|endoftext|>`, `<end_of_turn>`) and collecting all single-token IDs into a set. For Gemma 3: `<end_of_turn>`.

**Thinking mode suppression:** For Qwen3 models, `<think>...</think>` reasoning blocks are suppressed via `enable_thinking=False` in `apply_chat_template()` with a `TypeError` fallback for non-Qwen models. A regex safety net strips any residual think blocks from output. Not needed for Gemma 3 (current production model) but preserved for model-agnostic compatibility.

## Model Selection: Why gemma-3-4b-it-4bit

On 2026-03-31, four models were tested on the same bridge-length prompts (~4000 tokens of system prompt + history + journal + spectral summary):

| Model | Effective tok/s | 512-token time | Stable under coupling? | Failure mode |
|-------|----------------|----------------|:----------------------:|---------|
| gemma-3-4b-it-4bit | 55–69 | ~8s | **Yes** | — |
| Qwen3-8B-4bit | 18–34 | ~15–28s | No | Template-locking, degenerate loops |
| Qwen3-14B-4bit | 2.4–3.2 | ~160–210s | No | Prefill timeouts on bridge prompts |
| Gemma 2 9B | ~20–30 | ~20s | No | Degenerate output under coupling |

The critical finding is that **larger models degrade under bidirectional per-token coupling**, not just due to speed but due to behavioral instability. The coupling loop (embed_tokens -> project -> reservoir tick -> logit modulation) interacts poorly with larger models' internal dynamics, producing template-locked or degenerate output.

**Decision:** gemma-3-4b-it-4bit is the right model for this architecture. Fast enough (55-69 tok/s) that coupling overhead is negligible, and proven stable across thousands of coupled exchanges. Quality limitations are addressed through prompt hardening rather than model scaling.

## Bridge-Side Prompt Caps

The bridge trims prompts to fit the model's context window. Hardened on 2026-03-31 with explicit per-block caps and a global safety net.

| Setting | Value | File | Notes |
|---------|-------|------|-------|
| MAX_PROMPT_CHARS | 6,000 | `coupled_astrid_server.py` | Global safety net in mlx_chat() |
| System prompt | ~3,250 chars | `llm.rs` | Trimmed from 16,524 (80% reduction); action catalog compressed to one-liner-per-category |
| generate_dialogue() block caps | 800/400/400/800/800/300/300 chars | `llm.rs` | Per-section caps: system/journal/history/spectral/perceptions/interests/codec |
| Response length | 768 tokens | `autonomous/state.rs` | Soak safety cap, will be relaxed once proven |
| Gibberish gate | <40% alphabetic → reject | `coupled_astrid_server.py` | Catches degenerate MLX output |
| t_mod clamp | [0.5, 2.0] | `mlx_reservoir.py` | Prevents reservoir from forcing extreme temperature |
| ws.recv timeout | 5s | `coupled_astrid_server.py` | All reservoir RPC calls; auto-creates handle on timeout |

## State Flow Through a Generation

1. **Bridge sends request** — `/v1/chat/completions` with messages, temperature, max_tokens
2. **Pull reservoir state** — WebSocket to reservoir_service:7881, checks out "astrid" handle's `(h1, h2, h3)` vectors
3. **Format prompt** — `apply_chat_template(messages, enable_thinking=False)`
4. **Prefill** — process prompt tokens through the LLM (no reservoir coupling during prefill)
5. **Generate loop** (per token):
   - Sample token from logits (modulated by previous tick's y-values)
   - Extract token embedding from LLM: `embed_tokens(token)` → (1, 3072)
   - Project to reservoir input: `EmbeddingProjection.project()` → (1, 32)
   - Tick reservoir: `reservoir.step_multi(input, state)` → `(y1, y2, y3), new_state`
   - Feed y-values to `ReservoirLogitProcessor` for next iteration
   - Every 64 ticks: `mx.eval(*state)` to force Metal synchronization
6. **Push state** — evolved `(h1, h2, h3)` checked back into reservoir_service
7. **Post-process** — strip `<think>` blocks and `<|im_end|>` artifacts
8. **AGC update** — record y-values, adjust coupling_strength if variance is out of band
9. **Return response** — JSON back to bridge

## Launchd Configuration

**Plist:** `~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist`

```xml
<key>ProgramArguments</key>
<array>
    <string>/Users/v/other/neural-triple-reservoir/.venv/bin/python</string>
    <string>/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py</string>
    <string>--port</string>
    <string>8090</string>
    <string>--coupling-strength</string>
    <string>0.1</string>
    <string>--model-memory-map</string>
    <string>--model</string>
    <string>mlx-community/gemma-3-4b-it-4bit</string>
</array>
```

- `KeepAlive: true` — auto-restarts on crash (model load ~3s)
- `--model-memory-map` — memory-maps weights onto unified memory
- Logs: `/Users/v/other/neural-triple-reservoir/logs/coupled-astrid.log`

## Memory Impact

| Component | Memory |
|-----------|--------|
| gemma-3-4b-it-4bit weights | ~2.5 GB |
| KV cache (8K context) | ~0.5 GB |
| Reservoir MLX tensors | ~50 MB |
| Python process overhead | ~200 MB |
| **Total** | **~3.3 GB** |

On 64 GB M4: system-wide memory free is 80%+ with all 10 processes running.

## Known Bottleneck

The `ReservoirLogitProcessor` calls `mx.median(logits).item()` on every token for the h3 tail-scaling. The `.item()` forces a Metal→CPU synchronization — the GPU must finish computing the median before Python can read the scalar. This accounts for ~77% of per-turn coupling overhead.

**Mitigation options (future):**
- Precompute median on the previous logits (one-step lag, avoids sync)
- Replace median with a cheaper statistic (mean, percentile via sorting)
- Batch the sync points (accumulate then eval every N tokens)

## Related: Reflective Sidecar Model

The coupled server handles Astrid's **live generation** (dialogue_live). A separate MLX model handles **reflective analysis** during INTROSPECT: `gemma-3-12b-it-4bit` via `chat_mlx_local.py --model-label gemma3-12b`. This is a subprocess (not persistent), runs ~1 in 15 exchanges, and produces structured controller telemetry. See [Chapter 5](05-reflective-controller.md) for details.

These are separate model consumers with different requirements — the coupled lane needs speed (every exchange), while the reflective lane needs quality (rare, deep analysis). Changing one does not require changing the other.

## Model Upgrade Procedure

When changing Astrid's model:

1. Download new model: `snapshot_download('mlx-community/MODEL-NAME')`
2. Smoke test on alternate port: `--port 8091 --model NEW_MODEL`
3. Verify: hidden_size detected, stop tokens resolved, no `<think>` blocks, y-values in [-2, 2]
4. Back up coupling journal: `cp state/coupling_journal.json state/coupling_journal.OLD.backup.json`
5. Clear journal: `echo '{"version": 1, "entries": []}' > state/coupling_journal.json`
6. Update plist `--model` arg
7. Reload: `launchctl unload/load`
8. Rebuild bridge if prompt caps changed: `cargo build --release`
9. Monitor first 10 exchanges: tok/s, timeout failures, y-value ranges, Astrid's voice quality
10. Rollback: revert code + plist + restore journal backup + reload
