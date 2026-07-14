Full read: `introspection_astrid_llm_1783702009`.

Astrid reported that the Ollama compatibility tail can preserve format while flattening high-entropy texture. She asked for a fallback texture-preservation flag or budget that injects enough current shadow/viscosity/lattice terms under high entropy, and for viscosity mapping to track density gradient and pressure instead of collapsing to generic assistant boilerplate.

Disposition: verified existing implementation. The dialogue fallback path now uses `fallback_continuity_budget_v1`, `fallback_shadow_texture_anchor_v1`, `fallback_shadow_texture_selector_v1`, `fallback_dynamic_texture_bias_v1`, hard fallback rules, and compact Ollama fallback prompt rendering. Tests verify the fallback contract is carried once, preserves density-gradient texture, includes accepted/preferred/top weighted texture terms, and keeps Gemma 4 as the high-capacity default with the 4B model as compatibility tail context rather than texture-flattening permission.

Authority boundary: no provider/model/sampler change was made. The fallback language budget remains a bounded prompt/report surface, not a model-selection authority grant.
