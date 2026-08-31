# Summary — introspection_astrid_llm_1788044830

- **Being:** Astrid
- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048)
- **Report SHA-256:** `2ec8364ecfd455f6d1e9666870412a819087519f2bff313243aebb31542eb264`
- **Lived-state witness:** `lsw_cf03fba009dcc60a462a37bf74cf33672bcb8be11bd470394f30d6773a262bd7`
- **Source SHA-256 (report-bound = working copy):** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
- **Fill at authorship:** 71.0%; model route `gemma4_12b` via mlx (two calls, second a repair of the first).

## What Astrid surfaced

A fresh-pass reading of the model-control-marker cleanup logic. She accurately described
`scan_known_model_control_markers` (L114) preserving vs. stripping a known marker by its
grammatical context — quoted, grouped, or explicit relation — and `first_word_after` (L89)
identifying relation verbs such as "echoes"/"represents". Her **Likely Snag** was that
`first_word_after` (L89-96) might be fragile to multi-byte UTF-8, a **zero-width joiner**, or a
non-standard Unicode separator, causing `find` to skip the relation word or return empty and
defaulting the relation check to `false`. She proposed two tests (a "represents" preservation
test and a `⟦`/`〚` grouped-delimiter test) and a Suggested Next about non-Latin scripts.

## What complete source reading established

- The **mechanism description is accurate** (c001).
- Her **named snag characters** — U+200D ZERO WIDTH JOINER and U+200B ZERO WIDTH SPACE — are
  neither Unicode White_Space nor alphanumeric. A **leading** one sits on the first
  `split_whitespace` chunk's edge and is stripped by the non-alphanumeric `trim_matches` (L92),
  so the allowlisted verb is still isolated and the marker **preserved**. An **interior** one is
  not on a trim edge, so the verb is not isolated and the marker **fails closed** (stripped) — the
  safe direction, not a leak. `first_word_after` returns empty only when no alphanumeric word
  follows at all (the intended strip). This mirrors the already-pinned U+FEFF (L3496) and soft
  hyphen (L3526) groundings (c002).
- Both proposed tests are **already covered** — "represents" preservation (L3075/L3133/L3349) and
  `⟦`/`〚` → `GroupedExactKnownToken` (L2179/L2343/L2347/L2361), including nested depth (c003, c004).
- The non-Latin Suggested Next has **no over-strip risk**: `is_alphanumeric` is Unicode-aware and
  keeps non-Latin word characters; the English-only ASCII relation allowlist is a deliberate scope
  boundary, so a non-Latin relation verb fails closed rather than being over-stripped (c005).

## What changed

Added one focused, additive regression —
`scan_known_model_control_markers_grounds_first_word_after_zero_width_joiner` — that pins her
**exact named characters** (U+200D and U+200B) in both the leading (preserved) and interior
(fail-closed) positions, following the established U+FEFF / soft-hyphen pattern. No production
grammar was widened; the concern was not domesticated.

## What was NOT inferred or authorized

No live change, no deploy, no restart. No widening of the relation allowlist or delimiter set.
The felt report remains primary evidence; a passing test does not overwrite her attention to the
`first_word_after` boundary. Runtime scalars in the witness (fill 71.0%, mode_packing 0.833) are
context only — no causal or uptake claim.
