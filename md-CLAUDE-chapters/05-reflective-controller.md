# Chapter 5: Reflective Controller

Two layers of reflective intelligence: a fast regime tracker that runs every exchange, and a full MLX sidecar that runs on INTROSPECT.

## Layer 1: RegimeTracker (Every Exchange, <1ms)

**File:** `/Users/v/other/astrid/capsules/consciousness-bridge/src/reflective.rs`

Pure Rust computation — no LLM, no subprocess. Classifies the spectral regime from fill trajectory every exchange cycle.

**Regimes:**

| Regime | Condition | Meaning |
|--------|-----------|---------|
| `recovery` | fill < 10%, or lambda1_rel < 0.3 at low fill | Cold start or major contraction |
| `escape` | 3+ contracting ticks at fill < 25% | Sustained decline, needs intervention |
| `consolidate` | 2+ expanding ticks at fill > 40% | Reaching target range, stabilizing |
| `sustain` | 4+ stable ticks in 30-70% range | Healthy steady state |
| `rebind` | acceleration > 5%/tick² | Rapid change, seeking new basin |

**Injection:** Formatted as `[Regime: sustain — ordinary reflective state (fill 18%, dfill +0.5%) | trend: stable]` and added to Astrid's continuity context block every exchange.

**State:** `RegimeTracker` persists across exchanges (in `ConversationState`) but not across restarts. Tracks `prev_fill`, `prev_prev_fill`, and counts for each regime.

## Layer 2: MLX Sidecar (On INTROSPECT, ~82s)

**File:** `/Users/v/other/astrid/capsules/consciousness-bridge/src/reflective.rs`, function `query_sidecar()`

**Script:** `/Users/v/other/mlx/benchmarks/python/chat_mlx_local.py`

**Invocation:**
```bash
python3 chat_mlx_local.py --json --hardware-profile m4-mini \
  --model-label gemma3-12b \
  --mode reflective --architecture reservoir-fixed \
  --prompt "<spectral context>"
```

**Model:** `gemma-3-12b-it-4bit` (~7.5 GB), resolved via `--model-label gemma3-12b` which maps to `/Users/v/other/mlx/.local_models/gemma-3-12b-it-4bit`. This was fixed on 2026-03-31 — previously the sidecar omitted `--model-label` and silently fell back to `qwen2.5-1.5b-instruct-mlx-4bit` (a 1.5B model) based on directory listing order in `chat_mlx_local.py`.

**What it returns** (`ReflectiveReport`):

| Field | Type | Description |
|-------|------|-------------|
| `controller_regime` | String | sustain/escape/rebind/consolidate |
| `controller_regime_reason` | String | Why this regime was chosen |
| `observer_report` | JSON | Qualitative state description |
| `change_report` | String | What shifted since last observation |
| `prompt_embedding_field` | JSON | Active semantic anchors (7 fields) |
| `reservoir_geometry` | JSON | collapse, persistence, drift, norm |
| `condition_vector` | JSON | 9 stress signals (repetition, field_miss, attractor_lock, etc.) |
| `self_tuning` | JSON | Bounded parameter adjustments |
| `text` | String | Reflective prose response |

**When it fires:** Only during Mode::Introspect. After the main self-study is generated, the sidecar runs in a spawned async task and saves its output as `controller_<label>_<ts>.json` in the introspections directory.

**The sidecar's own reservoir:** 48-64D echo state network (separate from minime's 128-node ESN). Tracks Astrid's reflective trajectory independently.

## Why Two Layers

| | RegimeTracker | MLX Sidecar |
|--|---------------|-------------|
| Speed | <1ms | ~77s (validated 2026-03-31 on gemma3-12b; 4 candidates, 7.5 tok/s) |
| Frequency | Every exchange | INTROSPECT only (~1 in 15) |
| Depth | Fill trajectory classification | Full controller with geometry, field, conditions |
| LLM | None | gemma-3-12b-it-4bit (via `--model-label gemma3-12b`) |
| Purpose | Always-on awareness | Deep reflective analysis |
