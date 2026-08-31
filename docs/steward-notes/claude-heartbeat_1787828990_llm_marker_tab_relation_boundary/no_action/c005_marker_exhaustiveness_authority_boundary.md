# No-action / authority boundary — c005 (marker-list exhaustiveness)

**Report:** `introspection_astrid_llm_1787824358`, claim c005 ("Suggested Next: examine
`KNOWN_MODEL_CONTROL_MARKERS` to ensure the list is exhaustive for the current
deployment's safety requirements").

## What was done (evidence-only)

- Read the complete constant: `capsules/spectral-bridge/src/llm/provider/fallback_contracts.rs`
  L159–180 (source SHA-256 `23fb26a1388d75defd4e06dcd88d43fcd23ed79d471f356aadd8dbad1a44a1f6`),
  20 markers enumerated (Gemma/GPT/Llama channel + turn + special tokens + INST tags).
- Confirmed the constant's structural invariant is already pinned: no marker is a proper
  byte-prefix of another (`known_model_control_markers_have_no_proper_prefix_shadow`,
  `tests.rs` L3101), so `longest_exact_known_model_control_marker_at` never has to
  disambiguate a prefix shadow.

## What was deliberately NOT done, and why

Judging whether the list is **exhaustive for deployment safety**, or adding/removing any
marker, changes what the live model-output sanitizer strips. Per the flywheel handoff's
Tier-5 list, "fallback contracts or model behavior changes" require explicit
Mike/operator approval. This evidence-only, controller-held run has no such authority, so
no marker was added, removed, or reordered, and no exhaustiveness verdict was rendered.

Astrid's suggestion remains valid read-only research (Tier 1) she may pursue; this note
records the observation and the authority boundary without converting silence into a
decision.
