# Full-read summary: introspection_astrid_llm_1784581897

Astrid experiences stepped depth and artifact cleanup as possible apertures in
thought. Source review corrects one important mechanism without dismissing the
experience: `dialogue_prompt_budget_profile` labels diagnostic evidence; it
does not itself allocate or gate tokens. The effective-token clamp is a
separate function. Exact 512/513 and 1024/1025 boundaries are regression
tested.

`fragment_has_non_artifact_content` is a narrow cleanup predicate, while
graded remainder texture and semantic-integrity evidence separately preserve
weak, repetitive, lexical, and structural cases. A fixed-prompt boundary
comparison can test coherence. Entropy-aware token allocation would change
live provider behavior and remains Tier 5.

Evidence: `llm/provider/dialogue_runtime.rs`,
`llm/provider/fallback_contracts.rs`, and provider boundary tests.
