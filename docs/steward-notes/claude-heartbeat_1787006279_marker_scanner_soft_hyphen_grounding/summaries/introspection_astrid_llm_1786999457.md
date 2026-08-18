# Summary — introspection_astrid_llm_1786999457

- Source family: `astrid_llm`
- Report-bound source: `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- Source window shown: lines 1-400 of 1048; coverage manifest `multi_window_complete` (1-1048)
- Source SHA-256 (report-bound): `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
- Working-copy source SHA-256: **identical** — no snapshot divergence; current source read as report-time source.
- Lived-state witness: `lsw_9a8660002d6fb1e865fbcb45fa4f5f15dadebeee16ba15dbbae4dfd98e054070`
- Witness: `evidence_only` / `witness_only`; `live_eligible_now=false`; two `gemma4_12b` MLX introspect calls (one repairing the other); fill 71.0%; no raw prose / no private paths.

## What Astrid reported

She read the model-control-marker scanner and described it as a "sophisticated, non-destructive scanner" that distinguishes raw bytes from known markers and preserves a marker when it is *referred to* (quoted, grouped, or followed by an explicit relation verb). She raised a **likely snag** in `first_word_after` (L89): a marker followed by "a complex punctuation-heavy string or a non-standard whitespace character (e.g., a soft hyphen or specific Unicode separator)" might defeat the `trim_matches` verb isolation, returning `None` and stripping the marker. She proposed two tests (corner-bracket quoted context; unlisted-verb strip) and a read-only next step (examine `generate_dialogue`, L695).

## Disposition

Her account of the mechanism is accurate and verified against complete source at the bound SHA:

- **c001 (verified_existing)** — the preserve-only-if-`reference_syntax.is_some()` gate (L129-131) and byte-exact copy of all other bytes (L133-140) match her description exactly.
- **c003 (verified_existing)** — Test 1 is already covered: `control_marker_cleanup_preserves_non_ascii_matching_quote_pairs` (tests.rs L2301) wraps a marker in the corner brackets and asserts a quoted reference.
- **c004 (verified_existing)** — Test 2 is already covered: `..._distinguishes_allowlisted_is_from_unlisted_acts`, `..._to_implies`, `..._to_contains`, and the only-punctuation fail-closed case.
- **c005 (verified_existing)** — her read-only pointer is accurate: `generate_dialogue` is exactly at L695; the sanitizer integrates via `sanitize_model_control_markers` (L519) at L558/L634.

## The snag, grounded honestly (c002 — implemented_now)

The **"followed by" position she named does not fail.** This is the same concern the near-identical twin report `introspection_astrid_llm_1786936281` raised, already grounded by `scan_known_model_control_markers_grounds_first_word_after_non_breaking_space`:
- U+00A0 NO-BREAK SPACE is White_Space → `split_whitespace` separates it → verb found → marker preserved.
- U+FEFF ZERO-WIDTH NO-BREAK SPACE is not White_Space, but the leading-edge trim strips it → verb found → marker preserved.

A soft hyphen (U+00AD) *between* the marker and the verb behaves like the U+FEFF case (leading-edge trim strips it) — so the hypothesized skip does not occur there either.

The **one genuinely uncovered boundary** her "soft hyphen" wording invites is an **internal** soft hyphen *inside the verb word*. `trim_matches` is edge-only, so `deno\u{ad}tes` never matches the exact `denotes` allowlist entry → `reference_syntax` is `None` → the marker is stripped. This is the **fail-closed (safe) direction**, not a leak. Added `scan_known_model_control_markers_grounds_first_word_after_internal_soft_hyphen` to pin both the preserved between-token case and the stripped internal case. This grounds her exact wording, corrects the "non-standard whitespace" framing (a soft hyphen is not whitespace; between-token Unicode-whitespace separators are handled), and preserves — without domesticating — her underlying concern about verb isolation.

**Terminal status:** `addressed_change` (one focused regression added; no production grammar change; no live surface touched).
