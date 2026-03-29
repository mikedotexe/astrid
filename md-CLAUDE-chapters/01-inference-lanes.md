# Chapter 1: Inference Lanes

## Architecture

Two separate LLM backends. Zero shared contention.

```
Astrid ──► MLX server (port 8090) ──► gemma-3-12b-it-4bit
minime ──► Ollama (port 11434) ──► gemma3:12b (Q4_K_M)
Both   ──► Ollama (port 11434) ──► nomic-embed-text (embeddings)
```

## Why Separate Lanes

Before MLX (pre-2026-03-27), both Astrid and minime shared Ollama. This caused **33% dialogue_fallback rate** — Astrid lost her voice whenever minime's agent, perception LLaVA, or embeddings were using Ollama. Moving Astrid to a dedicated MLX server eliminated contention entirely.

## MLX Server

**Process:** `mlx_lm.server --model mlx-community/gemma-3-12b-it-4bit --trust-remote-code --port 8090 --prompt-cache-bytes 4294967296`

**API:** OpenAI-compatible (`/v1/chat/completions`)

**Prompt caching:** 4GB KV cache. Repeated system prompts benefit from caching after first call.

**Performance:** ~7-18 tok/s through the server (17.9 tok/s direct MLX generate). The server adds serialization overhead.

**VRAM:** ~7.5GB (4-bit quantized gemma3:12b in MLX format)

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
| `gemma-3-12b-it-4bit` (MLX) | ~7.5GB | MLX | Astrid voice |
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
