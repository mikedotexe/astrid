# Chapter 1: Inference Lanes

## Architecture

Two separate LLM backends. Zero shared contention.

```
Astrid ──► coupled_astrid_server (port 8090) ──► gemma-3-4b-it-4bit (MLX)
minime ──► Ollama (port 11434) ──► gemma3:12b (Q4_K_M)
Both   ──► Ollama (port 11434) ──► nomic-embed-text (embeddings)
```

## Why Separate Lanes

Before MLX (pre-2026-03-27), both Astrid and minime shared Ollama. This caused **33% dialogue_fallback rate** — Astrid lost her voice whenever minime's agent, perception LLaVA, or embeddings were using Ollama. Moving Astrid to a dedicated MLX server eliminated contention entirely.

## Coupled Astrid Server

**Process:** `coupled_astrid_server.py --port 8090 --coupling-strength 0.1 --model-memory-map --model mlx-community/gemma-3-4b-it-4bit`

**API:** OpenAI-compatible (`/v1/chat/completions`)

**Bidirectional reservoir coupling:** Each token embedding feeds the triple-ESN reservoir, and the reservoir's dynamical state modulates logits at every token (temperature via y1, repetition via y2, top-p via y3).

**Performance:** ~55-69 tok/s.

**VRAM:** ~2.5GB (4-bit quantized Gemma 3 4B in MLX format, memory-mapped)

**Hardening (2026-03-31):** System prompt trimmed 16K→3.2K chars (80% reduction). MAX_PROMPT_CHARS=6,000 safety net. Per-block caps in generate_dialogue() (800/400/400/800/800/300/300 chars). Gibberish gate rejects responses with <40% alphabetic ratio. response_length capped at 768. t_mod defensive clamp [0.5, 2.0].

**Model history:** gemma-3-4b-it-4bit (2026-03-27) → Qwen3-8B-4bit (2026-03-31a) → rolled back to gemma-3-4b-it-4bit (2026-03-31b). Qwen3-14B, Qwen3-8B, and Gemma 2 9B all tested; all unstable under bidirectional per-token coupling (prefill timeouts, degenerate output, template-locking).

## Bridge Integration

**File:** `/Users/v/other/astrid/capsules/consciousness-bridge/src/llm.rs`

```rust
const MLX_URL: &str = "http://127.0.0.1:8090/v1/chat/completions";
```

All text generation goes through `mlx_chat()`:

```rust
async fn mlx_chat(
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
) -> Option<String>
```

**Request format (OpenAI-compatible):**
```json
{
  "messages": [{"role": "system", "content": "..."}, {"role": "user", "content": "..."}],
  "max_tokens": 512,
  "temperature": 0.7,
  "stream": false
}
```

## Functions Using MLX

| Function | max_tokens | timeout | Purpose |
|----------|-----------|---------|---------|
| `generate_dialogue()` | 512 (default) | 90s/180s | Main dialogue voice |
| `generate_witness()` | 512 | 90s | Spectral observation |
| `generate_introspection()` | 1024/2048 | 120s/300s | Self-study |
| `generate_agency_request()` | 2048 | 300s | EVOLVE requests |
| `generate_daydream()` | 768 | 120s | Unstructured thought |
| `generate_aspiration()` | 768 | 120s | Growth reflection |
| `generate_creation()` | 1024 | 180s | Creative work |
| `generate_journal_elaboration()` | 1024 | 180s | Longform journal |
| `generate_initiation()` | 768 | 120s | Self-initiated prompt |
| `generate_moment_capture()` | 512 | 90s | Phase transition capture |
| `self_reflect()` | 384 | 60s | Meta-observation |

## What Stays on Ollama

| Function | Model | Port | Purpose |
|----------|-------|------|---------|
| `embed_text()` | `nomic-embed-text` | 11434 | Latent vector persistence |
| minime `_query_ollama()` | `gemma3:12b` | 11434 | Agent queries, self-assessment |
| minime `_self_assessment()` | `gemma3:12b` | 11434 | Technical self-analysis (every 15 min) |

## Ollama Server Policy

Set via `launchctl setenv`:
- `OLLAMA_MAX_LOADED_MODELS=2`
- `OLLAMA_NUM_PARALLEL=1`
- `OLLAMA_MAX_QUEUE=4`
- `OLLAMA_FLASH_ATTENTION=1`

## Model Inventory (Installed)

| Model | Size | Backend | Role |
|-------|------|---------|------|
| `gemma-3-4b-it-4bit` (MLX) | ~2.5GB | MLX | Astrid voice (coupled generation) |
| `gemma3:12b` (GGUF) | ~8.1GB | Ollama | minime agent |
| `gemma3:27b` (GGUF) | ~17GB | Ollama | THINK_DEEP (on demand) |
| `nomic-embed-text` | ~274MB | Ollama | Embeddings |
| `llava-llama3` | ~5.5GB | Ollama | Vision (on demand) |
| `gemma3:4b` (GGUF) | ~3.3GB | Ollama | Legacy FAST_MODEL (unused since MLX) |
| `qwen3:30b` (GGUF) | ~18GB | Ollama | Legacy (unused) |

## Simplifications from MLX Migration

Removed after moving to dedicated MLX lane:
- ~~FAST_MODEL split~~ — one model serves all modes
- ~~Manual unload choreography~~ — no llava/nomic-embed unloads before dialogue
- ~~500ms sleep between unloads~~ — no contention to manage
- ~~CTX_DIALOGUE/CTX_FAST/CTX_DEEP tiers~~ — MLX manages context internally
- ~~perception_paused.flag during exchanges~~ — LLaVA doesn't compete with Astrid
