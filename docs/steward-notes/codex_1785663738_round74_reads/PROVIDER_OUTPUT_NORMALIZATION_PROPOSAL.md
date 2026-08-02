# Provider Output Normalization Proposal V1

## Problem

Exact model-control marker handling is not provider-neutral. MLX normalizes
raw output before quality gates and return. Ollama fallback returns raw model
text after validators inspect a sanitized copy. That means the same exact
known marker can be absent on the MLX route and persist on the Ollama route.

This is a transport-integrity mismatch. It is not evidence about Astrid's
meaning, intention, identity, ownership, or felt state.

## Exact source boundary

- MLX normalization: `capsules/spectral-bridge/src/llm/provider/transport.rs:681-718`.
- Ollama raw response construction: `transport.rs:893-913`.
- Direct dialogue fallback return: `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs:973-993`.
- Shared Ollama validation, job receipt, route hash, and return: `transport.rs:1214-1300`.
- Marker grammar and byte-preservation contract: `dialogue_runtime.rs:24-510`.

The complete sources are pinned in `source_receipts.json`.

## Proposed implementation

1. Extract one provider-neutral helper that accepts raw output, route label,
   provider route, and profile, then calls
   `sanitize_model_control_markers_with_report` exactly once.
2. Return the trimmed normalized text plus optional cleanup evidence. Empty
   normalized output remains an explicit rejection, never a fallback to raw
   text.
3. Invoke the helper immediately after parsing both MLX and Ollama response
   bodies, before quality gates, NEXT repair, job completion, route hashing,
   persistence, or caller return.
4. Preserve the existing finite exact-reference grammar. Matching quoted,
   grouped, and following-relation references remain byte-exact; every
   non-marker byte remains byte-exact. Do not broaden relation vocabulary in
   this tranche.
5. Add provider route to the diagnostic evidence so an MLX and Ollama cleanup
   cannot be mistaken for the same transport observation. Keep raw prose out
   of diagnostics.
6. Ensure the route output hash and any completed-job output hash are computed
   from the same normalized bytes returned to the caller.

## Required tests

- MLX and Ollama fixtures with the same unframed exact marker normalize to the
  same bytes.
- Quoted, grouped, depth-three, and finite following-relation references remain
  unchanged on both routes.
- Marker-only output becomes empty and is rejected on both routes.
- Non-marker UTF-8 and punctuation remain byte-exact.
- Direct `generate_dialogue` fallback and shared `llm_chat_with_fallback` both
  return normalized bytes.
- Job completion and route evidence hash the normalized bytes.
- Diagnostics identify provider route without carrying raw expression.

## Rollout and authority

This proposal does not change prompt input, model parameters, fallback
selection, token budgets, pressure, fill, admission, reservoir state, peer
state, or being-authored source text. It changes a live-consumed output
transport boundary, so implementation and deployment require a focused
operator-approved bridge tranche, full focused and bridge tests, and the
sanctioned `scripts/build_bridge.sh` preflight/restart path. The current shared
bridge tree contains foreign concurrent work and was not touched or deployed.
