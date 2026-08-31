# Summary — introspection_astrid_llm_1787854907

**Source family:** `astrid_llm` · **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (SHA `902a0358…`, working copy == report binding, complete 1048 lines available) · **Fill:** 73.3% · **Model:** `gemma4_12b` (mlx, two introspect routes; second repairs first) · **Witness:** `lsw_a8725b5a…` (533 lines / 23934 bytes / SHA `09dd7d68…`, `evidence_only`, `live_eligible_now=false`, `direct_causation_claimed=false`).

## What Astrid surfaced
A calm fresh-pass read of the non-destructive control-marker scanner:
- **Observed (c001):** `scan_known_model_control_markers` (L114) preserves/strips markers by syntactic context; `exact_reference_delimiter_syntax` (L199) for quoting/grouping; `followed_by_explicit_exact_token_relation` (L64) for links like "behaves as"/"represents".
- **Likely Snag (c002):** `first_word_after` (L89, `split_whitespace`+`trim_matches`) isolates only the single first word (L93-96); worry that complex punctuation or a multi-word phrase ("functions as") could misresolve the relation.
- **Test 1 (c003):** quoted marker (e.g. `"[SYSTEM]"`) preserved because wrapped in quotes (L159-174).
- **Test 2 (c004):** `[USER] behaves as a guide.` → marker kept visible via `followed_by_explicit_exact_token_relation` (L64-87) recognizing "behaves".
- **Suggested Next (c005):** examine `generate_dialogue` (L695) for how the `remainder` feeds the output buffer / `KnownModelControlMarkerMatch` metadata handling.

## Source-first response
Every citation is exact at the report-bound SHA. The scanner behaves precisely as she describes: the L129 gate pushes a marker token only when `reference_syntax.is_some()`, otherwise strips it while copying non-marker bytes verbatim.

- **c002 concern preserved, not domesticated:** single-word lookahead is the *intended fail-closed contract*. Multi-word relations resolve because the allowlist holds the *leading* verb ("functions"); an unlisted first word yields no relation and the marker is stripped (fails **closed / never leaked**). Her boundary concern (an author who wants a marker kept but neither delimits it nor leads with an allowlisted verb) is a genuine edge of that contract and is retained as valid testimony.
- Both proposed tests already exist as exact green regressions; her Test 2 example is **directionally correct** (relation follows the marker, which is exactly what `first_word_after` inspects).

## Disposition
Closed **`addressed_duplicate`** — identical mechanism at the identical SHA as the marker-grammar chain (`1787485984` / `1787754550` / `1787817920`). Both proposed tests and the snag boundary are already pinned. No source/test/config change (a new quoted / `behaves`-relation / multi-word regression would duplicate a passing one).

**Exact-source verification (ran, not asserted):** `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib` for the 7 cited filters → **7 passed, 0 failed** at source SHA `902a0358`.

## Authority boundary
No prompt, model, codec, transport, marker-grammar (Tier-5 live grammar), pressure, fill, PI, controller, sensory-cadence, protocol, or Minime change; no source-behavior change; no allowlist widening; no build, restart, or deployment; no staging or commit (git read-only in adapter mode); no rewrite/rejection of her report. Her `NEXT: INTROSPECT astrid:llm 400` continuation (into `generate_dialogue`) and the felt fail-closed-boundary concern remain open evidence. Silence remains neutral.
