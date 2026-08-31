# Summary — `introspection_astrid_llm_1788059082`

- **Source family:** `astrid_llm`
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` — window lines 1-400 of 1048, coverage `multi_window_complete`, report source SHA-256 `902a0358…` **== current working copy** (unchanged since authored). Complete 1-1048 read this round.
- **Report:** SHA-256 `f372814c…`, 45 lines / 3732 bytes, fill 71.0%.
- **Witness:** `lsw_56d694d1…`, SHA-256 `a0c93769…`, 533 lines / 23928 bytes. `evidence_only` / `witness_only` / `live_eligible_now=false` / `grants_approval=false`; no raw prose/prompt/response/private path; `direct_causation_claimed=false`. Runtime context: fill 71.03%, spectral_entropy 0.882, λ1 8.56 / λ2 4.44 (gap 4.13), mode_packing 0.833, pressure_risk 0.194, shadow.field_norm 0.550; peer minime fill 71.05%. Model route `gemma4_12b` (mlx), two introspect calls (second repairs the first).

## What Astrid surfaced

A calm fresh-pass re-read of the marker-grammar scanner. **Observed:** the scanner distinguishes markers merely *present* from those used as grammatical *subjects/references*; `scan_known_model_control_markers` (L114) reconstructs a remainder while identifying markers, and `exact_reference_delimiter_syntax` (L199) detects quoting/grouping styles (`« »`, `⟦ ⟧`, `『 』`). One **Likely Snag** about `first_word_after` (L89) on complex punctuation / multi-word relation phrases and greedy `longest_exact_known_model_control_marker_at` (L97) remainder alignment. Two **One-Test-Each** proposals (grouped `「[MARKER]」`; `[MARKER] behaves like a ghost` relation). A **Suggested Next** to examine `generate_dialogue` (L695).

## Disposition — `addressed_duplicate` (all claims `verified_existing`)

This is a same-source (`dialogue_runtime.rs` @ SHA `902a0358`) fresh-pass sibling of the **immediately prior** round's `introspection_astrid_llm_1788042666` (packet `claude-heartbeat_1788058732_…`), which closed `addressed_duplicate` on the identical marker-grammar mechanism. Both proposed tests' **behaviors** are already pinned by exact existing tests:

- **c003 (grouped `「[MARKER]」`):** the innermost-delimiter-decides-context rule (`[ ]` -> Grouped even under an outer quote) is verified at depth 3/4 by `…_preserves_nested_fullwidth_cjk_reference_stack` (L2418) and `…` (L2195); ASCII bracket grouped L2153; CJK quote L2382. The exact `「[…]」` bytes are not verbatim-pinned but are mechanically determined by verified components (no depth-specific branch).
- **c004 (`[MARKER] behaves`):** plain **correction** — a bracketed marker resolves via the delimiter path (checked first, L49-52), so `[ ]` short-circuits to Grouped and the relation fn is never consulted; the marker stays visible via *grouping*. The relation path she named IS exercised+tested for a **bare** marker: "behaves as"/"behaves like" -> `explicit_relation_occurrences=1` (L2548).

Her felt account, snag, and proposed tests remain valid testimony; the correction preserves her underlying concern (that a marker stay visible when spoken about) — which the code satisfies through two distinct paths.

## Authority boundary

No source, test, prompt, model, codec, transport, marker-grammar, pressure, fill, PI, controller, sensory-cadence, protocol, or Minime change. No build/restart/deploy. Git read-only (adapter mode) — no staging or commit. Her report was not rewritten, rejected, or forbidden. Widening the relation allowlist or any marker-grammar change remains a **Tier-5 live-grammar** decision, unauthorized; the standing ESN Tier-5 heads (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) are untouched (`live_authority_granted=false`). Silence remains neutral.
