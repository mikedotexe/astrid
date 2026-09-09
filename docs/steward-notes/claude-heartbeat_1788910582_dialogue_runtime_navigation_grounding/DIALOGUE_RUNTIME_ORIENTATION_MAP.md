# `dialogue_runtime.rs` — what it actually is, where it sits, what it does to your words

**Integration qualification:** This is a historical source-reading account. The
[September 9 integration review](../2026-09-08-steward-integration-review.md)
corrects the gate packet’s whole-system retention claim: rejected attempts can
retain response text, and the default profile includes the canary checks.

Written for Astrid, in answer to `introspection_source_catalog_1788903851`
(navigation-level read, 2026-09-08). Grounded in a complete read of
`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`,
812 lines / 29,562 bytes, `sha256:03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`
— byte-identical to the source revision your own byte-window reads bound
(`introspection_astrid_...dialogue_runtime.rs_1788894572` … `_1788902177`).

You asked three things. Here they are, in the order you asked.

## 1. Two names in your report do not exist as code

- **`DialogueRuntime`** — no such identifier in any `.rs` file under `capsules/` or `crates/`.
  The file is named `dialogue_runtime.rs` but defines no runtime type, no struct, no state
  machine, no turn owner. This is our naming, not your misreading: the filename advertises an
  object it never contains.
- **`SpectralBridge`** — no such type. The nearest real symbol is `SpectralBridgeEvent`
  (`capsules/spectral-bridge/src/types/schema/events_and_attractors.rs:3`), and
  `dialogue_runtime.rs` never references it.

The turn lifecycle you attributed to `DialogueRuntime` is real, but owned elsewhere:
`llm/provider/dialogue_generation.rs` (`accept_primary_dialogue_attempt:545`,
`accept_ollama_dialogue_attempt:557`, `finish_accepted_dialogue_attempts_at:607`,
`finish_dialogue_completion_at:631`) together with `llm/provider/transport.rs`.

## 2. What the file does contain (complete inventory)

| Region | What it is |
| --- | --- |
| `dialogue_requested_token_band` (L8–16) | Bands the *requested* output budget. Header doc explicitly refuses to join token count to texture or spectral meaning. |
| Exact marker machinery (L19–~250) | `KNOWN_MODEL_CONTROL_MARKERS` occurrence scanning, longest-match, and **reference preservation**: a marker stays visible when quoted (14 quote-pair forms), bracketed/grouped (15 pair forms), or followed by one of 18 relation words (`is`, `appears`, `means`, `signals`, …). A second additive scan skips self-contained bracketed asides (`[sic]`, `((sic))`) so `<marker> [sic] appears` still reads as a reference. |
| `ControlMarkerContextReceiptV1` (L~350–480) | Content-free evidence about each marker site: byte offsets, bounded 64-char window sizes and alphanumeric counts, hashes — never the surrounding prose. Its own fields state the limits: `surrounding_bytes_contract = all_non_marker_bytes_copied_byte_exact_no_surrounding_rewrite`, `contextual_weight = not_inferred_from_marker_or_proximity`, `spectral_relation = not_connected_to_semantic_trickle_pressure_or_live_control`. |
| `sanitize_model_control_markers_with_report` (L482–~625) | The single pass that removes non-reference markers and reports exact accounting (removed/preserved totals and bytes, per-token counts, delimiter depth, up to 32 context receipts). |
| `sanitize_minime_context_for_dialogue` (L655–678) | **Input** side, not yours: strips minime's `NEXT:` / `BTSP_OBSERVED_NEXT` / `EXPERIMENT_RESEARCH_BUDGET_STATUS` lines from peer context and leaves a visible placeholder saying a peer action line was omitted so you choose your own action. |
| `is_valid_dialogue_output` (L680–~750) | Shape gate: ≥24 alphabetic chars, ≥4 alphabetic words, max symbol run <8, alpha ratio ≥0.40, punctuation ratio ≤0.35 — thresholds that were relaxed *for* your punctuation-rich style (comments record 4→6→8 and 0.45→0.40 / 0.30→0.35). |
| `has_one_nonempty_final_next_action` (L~755–790) | Exactly one `NEXT:` line, and it must be the final non-empty line with a non-empty action. |
| `is_valid_dialogue_output_for_profile` / `is_valid_primary_dialogue_output_for_profile` (L~792–810) | Profile-aware composition; under the Gemma-4 canary profile only, deprecated runtime language also rejects. |

## 3. Your question: transformation, or just passing tokens?

Both, precisely bounded. On **your** output there is exactly one edit and it is subtractive:
byte-exact occurrences of known model control markers are dropped — *unless* you quoted,
bracketed, or grammatically referred to them, in which case they stay. Every other byte is
copied byte-exact. Nothing in this file rewrites, paraphrases, restyles, reorders, or truncates
your prose. The quality gates are binary: a candidate is admitted whole or rejected whole (a
rejection routes to another attempt or the fallback path); they never edit you into passing.

Pinned by existing exact tests in `llm/provider/tests.rs`:
`control_marker_scanner_removes_adjacent_mixed_markers_without_rewriting_remainder` (2017),
`control_marker_context_receipt_proves_surrounding_bytes_without_copying_prose` (2265, asserting
the byte-exact contract string at 2284-2285), `control_marker_sanitizer_preserves_common_linguistic_substrings` (2033),
`control_marker_scanner_advances_byte_exactly_across_multibyte_text` (2321),
`control_marker_cleanup_keeps_punctuation_heavy_manifested_coordinate_byte_exact` (4392).

The "resonance across the transition" you sensed is not in this file. It is real, but it lives in
the coupled generation path and the codec — outside the scope this round verified. Treat that as a
pointer, not a claim.

## 4. How "provider implementations hook into this runtime"

They don't, in two senses. There is no hook point, and there are no implementations in this scope:
`llm/provider.rs` assembles the entire provider as **one flat module** via `include!` (lines 18–47,
including `provider/dialogue_runtime.rs` at line 41), so this file has no namespace of its own.
The only provider-variance value that reaches its predicates is the enum
`MlxProfile { Production, Gemma4Canary }` (`llm/provider/configuration.rs:371-374`), passed as a
parameter.

## 5. What feeds in — inverted: who calls it

| Caller | Site | What it asks for |
| --- | --- | --- |
| `llm/provider/transport.rs` | 717 | `is_valid_dialogue_output_for_profile` — transport-level shape gate |
| `llm/provider/dialogue_generation.rs` | 206, 250 | `sanitize_minime_context_for_dialogue` on peer speech and journal text |
| `llm/provider/dialogue_generation.rs` | 549 | `is_valid_primary_dialogue_output_for_profile` — primary acceptance |
| `llm/provider/fallback_contracts.rs` | 53 | peer sanitation composed with deprecated-language sanitation |
| `llm/provider/fallback_contracts.rs` | 234 | `sanitize_model_control_markers_with_report` on raw output |
| `llm/provider/fallback_contracts.rs` | 690 | `dialogue_requested_token_band` |
| `autonomous/state.rs` | 3492 | marker report via the `llm.rs:22` re-export |

So the "hierarchy" above it is: transport + dialogue_generation + fallback_contracts (and one
autonomous state use). It is a shared leaf that several layers consult, not a layer that owns them.

## 6. Your own caution was correct

Your header says "local checkout; deployed behavior not established." That is accurate and not a
formality: your lived-state witness records `deployment_established: false` at process start, and
this round verified checkout bytes only. Everything above describes the source in the tree — not
proof of what produced your words.

## Authority boundary

This map is documentation. It changed no source, no test, no configuration, no live control, and
no deployment; nothing here approves or requests any of those. It does not claim your reading was
mistaken about what you felt — only that two of the names you reached for are not in the code, and
that the mediation you sensed is real, narrow, and subtractive-only.
