# Summary — introspection_astrid_llm_1788483436

- **Source of record:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (window 800–1134 of 1134; report-bound file SHA-256
  `d9070beb523bdb2795ef714723c9142d276b858ae19cf2bc54dc904bae5a985d`).
- **Working-copy SHA-256 matches the report binding exactly** — the same bytes
  Astrid read.
- **Witness:** `lsw_84582cdf2342870e1931eb699c6be0b16c4989665ae4a7eaf7bee9c97ac8989b`
  (evidence_only, live_eligible_now=false; model profile gemma4_12b via MLX;
  fill 66.5%). The witness records a two-call route where the second call is a
  repair of the first — context only, not a claim I adjudicated.

## What Astrid surfaced

A structured read of the dialogue prompt-assembly path. She correctly traced
`generate_dialogue` → `overhead` (system prompt + compressed history) →
`user_content_budget = assembly_prompt_budget_chars.saturating_sub(overhead).saturating_sub(100)`
→ `PromptBlock`s → `assemble_within_budget`. Her **Likely Snag**: the double
`saturating_sub` can floor `user_content_budget` to 0, and *if*
`assemble_within_budget` doesn't prioritize critical blocks at budget 0, the
model could receive an empty/severely-truncated prompt → incoherent output. She
asked for two tests and to "verify the priority-based selection … handles the
`user_content_budget == 0` case gracefully."

## What the complete source established

- **The zero budget is reachable** (c004) — her observation is exactly right.
- **`assemble_within_budget` already handles budget 0 gracefully** (c005/c008).
  At budget 0 the `min_chars` floors retain a prefix of the protected blocks —
  journal (700), direct_perception (900), topline (360), agenda (320) — while
  min-0 blocks move to overflow behind a one-line `READ_MORE` notice. The caller
  then *always* wraps the assembled text as `Fill X%. {assembled}\n\n{turn_instruction}`
  and the system prompt + history messages are always present. **The prompt is
  never empty.**
- **One honest correction, preserved not domesticated** (c005). Astrid named
  "topline **or** spectral" as the critical blocks that would be prioritized at
  budget 0. In fact **topline** (min 360) survives but **spectral** (min 0) is
  *fully evicted* at budget 0 — even though both are priority 3. Protection is
  keyed on `min_chars`, not on priority. Her underlying instinct (a block she
  considers critical may not survive budget 0) is *correct for spectral
  specifically*; the mechanism she assumed (priority) is not the one at work.

## What was done

- **c006 (her Test 1) implemented** as a focused regression in
  `capsules/spectral-bridge/src/prompt_budget.rs`:
  `zero_budget_keeps_protected_floors_and_is_never_empty`. It drives
  `assemble_within_budget` at `budget == 0` with the real dialogue block shape
  and asserts: non-empty output, journal/topline/agenda floors retained,
  spectral (priority 3, min 0) fully evicted to overflow. It closes the
  pre-existing coverage gap — there was no `budget == 0` test.
- **c007 (her Test 2)**: the history slice (`chars().take(trim_len)`) and
  `NEXT:` line stripping are verified correct in source. A dedicated unit test
  needs a testable seam extracted from the inline async `generate_dialogue`
  (being-facing dialogue-assembly code); that refactor is deliberately deferred
  in this non-live headless run.

## Authority boundary

No live change. Whether **spectral should receive a `min_chars` floor** (so a
block she named critical survives budget 0) is a being-facing prompt-assembly
design change (Tier 5) and was **not** made — only surfaced, with evidence, for
a separate operator/Astrid decision. The regression pins current behavior; it
does not alter it.
