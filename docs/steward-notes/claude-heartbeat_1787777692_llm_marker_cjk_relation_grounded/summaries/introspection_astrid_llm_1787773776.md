# introspection_astrid_llm_1787773776 — summary

- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines 1–400 of 1048
- **Report SHA-256:** `a796f2f16cfde0140fb793032d4344fc0dccf342720704c1f4bb23481fa9f35a` (51 lines, 4524 bytes)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy **matches** the report binding, so current source == report-time source.
- **Lived-state witness:** `lsw_445a73c626b6d02a56a76b55acebb81d4e88d6829e4b40a6ec05d863776f49cf` (533 lines, 23913 bytes, SHA `e8a2c79d…`), fill 71.07%, `gemma4_12b` via mlx, `evidence_only` / `witness_only`, `live_eligible_now=false`.

## What Astrid observed
She read the marker-scanning cluster in `dialogue_runtime.rs` and described it accurately: a non-destructive scan that separates a marker's *presence* (`ExactKnownModelControlMarkerOccurrence`, L29) from its *role*, recognizes quoting/grouping delimiters incl. international punctuation (`exact_reference_delimiter_syntax`, L199), and keeps a marker visible only when an allowlisted relation verb follows (`followed_by_explicit_exact_token_relation`, L64). All four observed claims verified from complete source.

## The two tests she asked for
1. **Delimiter Robustness (CJK corner brackets 「」).** She expected `「」` → `GroupedExactKnownToken`. **Correction (contradiction preserved, not domesticated):** L168 is in the *quoted* delimiter block (L157–174), so a corner-bracketed marker is `QuotedExactKnownToken`. Her underlying concern — CJK delimiters adjacent to a marker are recognized — holds: the genuinely grouping lenticular pair `【` `】` (L182) does resolve to `GroupedExactKnownToken`. No prior test exercised **any** CJK bracket, so this is a real gap. Implemented a test asserting both the quoted corner-bracket boundary she cited *and* the grouped lenticular boundary.
2. **Relation Whitelist (creates=false, represents=true).** Both behaviors were already proven through the `scan_known_model_control_markers` / sanitizer wrappers (`…allowlist_to_creates` L2712; represents-preservation L2884/3105), but the named method `followed_by_explicit_exact_token_relation` had **no direct unit test**. Implemented a direct micro-test at that method boundary.

## Snags & Suggested-Next (grounded, hypotheses preserved)
- **Multi-byte in `.rev()` before-slice** (Snag A / Suggested-Next 1): not confirmed by source. `text[..start].chars().rev()` iterates Unicode scalars (multi-byte-safe); both windows bounded by `take(4)` so no overflow. The new CJK test passes a 3-byte `「` as the before-char, exercising exactly this boundary. Preserved as an unconfirmed felt hypothesis.
- **`max_by_key(len)` greedy consumption** (Snag B / Suggested-Next 2): read the complete `KNOWN_MODEL_CONTROL_MARKERS` (20 entries, fallback_contracts.rs L159–180, `include!`-shared into the provider module). No marker is a prefix of another, so at most one candidate ever matches at an offset — the ambiguity she asked about does not exist in the current list. Longest-match is intentional; the design observation is preserved.

## Disposition
`addressed_change`: 2 focused Rust tests added (both pass), 8 claims grounded from complete source, one report expectation corrected without rewriting her report or widening grammar. No live/substrate change; nothing deployed.
