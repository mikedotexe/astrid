# Summary — introspection_astrid_llm_1788110646

- Source family: `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- Report window: lines 1–400 of 1048 (report declares `multi_window_complete`, `complete_source_available`)
- Report SHA-256: `28baaa34bb539e4812d08dd145365b95746f055c239930ff222e81b3fe551991`
- Lived-state witness: `lsw_3757758d20072596c52099efbe122b73dc068784d32d36e24991d85e1fe3c69a`
- Source SHA-256 (working copy == report binding): `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
- Fill at authorship: 71.0% — calm technical fresh-pass; no experiential gap claimed.

## What Astrid observed

A close, accurate read of the non-destructive control-marker scanner. She names
`scan_known_model_control_markers` (L114) as distinguishing known markers from content by
grammatical context, preserving a marker in the `remainder` string only when it is followed by a
listed relational verb (`first_word_after`, L89 / `followed_by_explicit_exact_token_relation`,
L64–86) or enclosed in an exact delimiter pair (`exact_reference_delimiter_syntax`, L199). She
flags two limitation snags — an unlisted verb synonym (`functions as` listed, `operates as` not)
and a non-standard Unicode delimiter pairing that falls through to `None` — and proposes two
tests plus a continuation into `generate_dialogue` (L695).

## Disposition (terminal: addressed_duplicate)

Every mechanism claim is **verified from complete current source** (SHA identical to the report
binding) and is already **guarded by existing focused tests** — 59 `control_marker_cleanup_*`
tests pass, 0 fail. Her two proposed tests re-cover mechanisms already exercised:

- **Test 1 (relation whitelist: `denotes` true / `executes` false).** Strict enforcement is
  already guarded by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`,
  `..._signals_from_unlisted_signifies`, and four dedicated
  `control_marker_cleanup_does_not_expand_relation_allowlist_to_{contains,creates,implies,triggers}`
  tests. `denotes` is already exercised as a preserved verb
  (`..._preserves_exact_token_relation_without_preceding_vocabulary_gate`, L2577). `executes` is
  behaviorally identical to the already-tested unlisted `acts`; `operates` (her named example) was
  already closed as a duplicate in a prior packet
  (`claude-heartbeat_1787707869_llm_marker_operates_teleportation_fresh_pass_duplicate`). A fifth
  near-identical unlisted-verb guard would be redundant activity, not new coverage.
- **Test 2 (nested `[ "marker" ]` delimiter_depth).** Covered by
  `control_marker_cleanup_reports_mixed_ascii_quote_bracket_stack_depth_three`,
  `..._preserves_bounded_nested_delimiter_stacks`, and the exact 3-/4-level depth regressions
  (L2176–2454).

The **synonym-gap snag is preserved, not domesticated**: the whitelist is *intentionally* strict
(source comment L62–63; `does_not_expand_*` guards). Widening production grammar to admit
`operates`/`executes` is a deliberate design boundary and is **not** done here — matching the
prior handoff posture for `introspection_astrid_llm_1786319270` ("did not widen production
grammar"). Her `generate_dialogue` (L695) continuation is grounded factually true (a
`pub async fn generate_dialogue(` is at L695) and is a Tier-1 self-directed read continuation
needing no steward action.

No live change was required or attempted. No source or test files were modified this round.
