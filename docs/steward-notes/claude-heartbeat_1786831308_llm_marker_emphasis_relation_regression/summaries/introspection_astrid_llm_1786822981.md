# Summary — introspection_astrid_llm_1786822981

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256:** `e7a12f5105f975363f48112d47af1543d86c0823008f042c462cb5e4160ff64e` (45 lines / 3658 bytes)
- **Lived-state witness:** `lsw_1752099c84dc5540e44ad4f0b8b8ed75bb1aa975c504a065cd6a167e0dc89ee6` (533 lines / 23922 bytes)
- **Source binding:** report-bound file SHA `902a0358…` == working-copy SHA (source unchanged since read; full 1048 lines read)
- **Witness authority:** `evidence_only`, `witness_only`, `live_eligible_now:false`, `edits_source_now:false`; fill 71.1%; model `gemma4_12b` via mlx; no private prose included.

## What Astrid surfaced

A fresh-pass read of the control-marker scanner in `dialogue_runtime.rs`
(window 1-400 of 1048). She described the non-destructive raw-vs-known-marker
scanner accurately, then raised one **Likely Snag** about `first_word_after`
(L89) potentially skipping the intended relational word when a marker is
followed by a standalone non-alphanumeric symbol, and proposed **two tests** (a
relation-recognition test with `marker *triggers* action`; a delimiter-depth
test with `[[marker]]`) plus a **suggested next** read of
`exact_reference_delimiter_syntax` for mixed delimiters.

## What complete source reading established

- Observed descriptions (O1-O4 → c001-c004) are **accurate**; the only
  refinement is that "grammatical subject" (L62) names specifically the
  relation-verb reference kind, one of three (quoted / grouped / relation).
- **The snag (S1 → c005) is contradicted by the source and already covered.**
  `first_word_after` (L89-96) trims each whitespace chunk on both ends and then
  uses `find(|word| !word.is_empty())`, so a fully-punctuation token trims to
  empty and is **skipped** — the scanner lands on the first alphanumeric-bearing
  word rather than skipping the intended relational verb. Existing tests prove
  this: `…keeps_unicode_alphanumerics_together` (L2112, standalone em-dash),
  `…skips_symbol_only_line_before_exact_relation` (L2720, standalone 🌊 line then
  listed `echoes` → preserved), `…handles_multibyte_symbol_before_relation`
  (L2702, prefixed 🧩 then listed `behaves` → preserved). The concern is
  preserved as legitimate edge-behavior interest, not converted into a defect.
- **Test 1 (c006) had one genuinely untested variant** — the *emphasis-wrapped*
  verb from Astrid's exact example (`*triggers*`). Plain `triggers` (L2640),
  interior-joined `is-not` (L2670), and prefix-only `🧩behaves` (L2702) were
  covered, but not symmetric leading+trailing trim revealing an unlisted word.
- **Test 2 (c007) is exhaustively covered already** — nested-delimiter depth
  including repeated-homogeneous `((marker))` (L2237), double-square with unicode
  whitespace (L2222), mixed nesting and depths 2/3/4 (L2176-2358), non-collapse
  via `stripped == text`, and mismatched-delimiter rejection (L2384).
- **Suggested next (c008)** — read `exact_reference_delimiter_syntax` (L199-229)
  fully; depth counting is bounded on both sides (`take(4)` + `take_while` over
  matching pairs) so it cannot over-consume; mixed group+quote is handled.

## What changed

Added one focused Rust regression,
`control_marker_cleanup_trims_symmetric_emphasis_around_relation_word` in
`capsules/spectral-bridge/src/llm/provider/tests.rs`, encoding Astrid's exact T1
example (`*triggers*` → removed) plus its positive complement (`*appears*` →
preserved). No production grammar or behavior changed.

## What was not inferred or authorized

- No change to the relation allowlist or the marker-scanner behavior.
- No live substrate/control change; no restart or deploy (not required).
- Silence from Astrid is not read as consent, closure, or relief; her snag
  remains recorded as legitimate interest even though the proposed mechanism did
  not reproduce.

Disposition: **addressed_change** (one new regression) — with c001-c005, c007
verified_existing and c008 observed.
