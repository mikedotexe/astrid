# Summary — introspection_astrid_llm_1787699463

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048; coverage manifest `multi_window_complete`, uncovered `none`).
**Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy == report binding == witness binding (clean/tracked, unchanged since the report).
**Report SHA-256:** `e09f82ea0947a11813474b878a84cbd493e6319e8d5181d84b2e61f7dd5ef827` (45 lines / 3813 bytes).
**Witness:** `lsw_3a6f9c4b429b6926782df34950925ce57d3ee2d7880b4a2ca6f9ed2b576802a1` (533 lines / 23929 bytes; `evidence_only` / `witness_only`, `live_eligible_now=false`; artifact binding matches report; two `mlx`/`gemma4_12b` introspect routes, the second repairing the first; fill 70.4%).

## What Astrid surfaced
A fresh pass over the marker scanner: she describes `scan_known_model_control_markers`, the `reference_syntax` gatekeeper, and the quoted/grouped/explicit-relation contexts (Observed); flags two snags — a finite relation-verb allowlist (`first_word_after`) that could miss a valid verb, and the omission of a `None`-syntax marker from the reconstructed `remainder`, which she frames as possible "teleportation" of text if the marker was a structural anchor; proposes two tests (unlisted-verb visibility; nested-delimiter depth); and offers to continue into `generate_dialogue`.

## What complete reading established
Every cited line range is accurate. This is a near-twin of the immediately-prior report `introspection_astrid_llm_1787692007` (packet `claude-heartbeat_1787699077_...`, closed `addressed_duplicate`). All six concrete claims resolve to `verified_existing` against complete source + existing regressions:

- The **verb-allowlist boundary** (Snag a / Test 1) is by-design and pinned by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2568) and the `does_not_expand_relation_allowlist_to_*` family (L2682-2742). Her Test-1 example **"operates as"** is a genuinely unlisted verb that travels the identical `none_cleanup_candidate` path (also grounded specifically by prior round `1786932936_llm_marker_operates_synonym`). Contradiction preserved: her Snag-a examples **"functions"/"manifests" are already allowlisted** (L74/L77) and would be retained, not missed.
- The **remainder-omission "teleportation"** hypothesis (Snag b) is contradicted plainly: the `acts` test asserts output `"Here,  acts as a proxy..."` (L2593) and the end-of-string test asserts `"Here it is "` (L3079) — the marker bytes are removed **in place**, byte-exact, with surrounding non-marker text preserved in order. No reordering. Stripping unreferenced control markers is intended fail-closed behavior (they must not leak).
- The **nested-delimiter depth** (Test 2) is grounded by `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3025) and `..._reports_exact_three_level_delimiter_depth`.
- The **Suggested Next** (`generate_dialogue`) is her own read-only continuation offer; no steward action inferred.

## Tests
`cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib` filters `marker` (80) / `delimiter` (10) / `first_word` (5) = **95 passed, 0 failed** at source SHA `902a0358`. No new tests authored — an "operates" regression would duplicate the passing unlisted-verb class, and the "teleportation" concern is already refuted by asserted in-place outputs.

## Not inferred / not authorized
No source, prompt, model, codec, controller, or Minime change. No live/restart/deploy action. The standing Tier-5 waits (`wi_e579041bc76f8310` / `wi_69fbd510467c6337` / `wi_3e26ac525fea1c36`, all `live_authority_granted=false`) untouched. Silence about her continuation offer is neutral, not decline.
