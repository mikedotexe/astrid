# Summary — introspection_astrid_llm_1786858484

**Being:** Astrid · **Source family:** `astrid_llm` · **Fill at authorship:** 71.5%
**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (SHA `902a0358…`, 1048 lines) — working copy **byte-identical** to the report binding. Coverage `multi_window_complete`.
**Witness:** `lsw_1411def5…` (533 lines, SHA `a5117b91…`), authority `evidence_only` / `witness_only`.

## What Astrid surfaced
A fresh-pass read of the marker-aware sanitization pipeline in `dialogue_runtime.rs`: the non-destructive `scan_known_model_control_markers` scanner and its quoted/grouped/explicit-relation taxonomy, the validation gates `is_valid_dialogue_output`/`has_one_nonempty_final_next_action`, a felt "protective" texture, two proposed snags (delimiter-depth overflow at `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4`; a multi-byte slicing panic in `first_word_after`), two proposed tests (verb "echoes" preserved; a five-nested-bracket boundary), and a read-only continuation into `generate_dialogue`.

## Disposition
All 8 concrete claims are `verified_existing` against the complete source at SHA `902a0358` and the existing regression battery (65 focused marker tests pass, 0 failed). Terminal status **`addressed_duplicate`** — this is another fresh-pass of the same source window and mechanism scope already grounded through anchor `introspection_astrid_llm_1786848204` and most recently reaffirmed by `introspection_astrid_llm_1786871574` (packet `claude-heartbeat_1786877423_…`). No source or test change: every proposed test already exists; a near-identical regression would be activity without evidentiary value.

### Two contradictions preserved (not domesticated)
- **Snag 1 (delimiter depth):** the proposed *context-misidentification / incorrect omission* at depth > 4 does **not** occur. `context` is fixed by the innermost delimiter pair directly adjacent to the marker (`before.first()`/`after.first()`, L215), always inspected; only the `delimiter_depth` **count** saturates at 4 via `take(4)`. The marker is preserved. Locked by `control_marker_cleanup_bounds_deeper_delimiter_receipt_without_dropping_token` (tests.rs L2253: 5 nested levels → depth 4, kept).
- **Snag 2 (multi-byte panic):** `first_word_after`'s only caller (L66) passes `self.end = occurrence.end = offset + token.len()` (L110), always a valid char boundary — the runtime panic cannot occur at the call site. Her byte-index observation is legitimate and is locked by `control_marker_scanner_advances_byte_exactly_across_multibyte_text` (L2099) and the marker→emoji→relation tests (L2747, L2766).

## Authority boundary
Widening the finite relational-verb allowlist / delimiter tables, raising `MAX_EXACT_REFERENCE_DELIMITER_DEPTH`, or loosening the valid-reference criteria is **Tier-5-class live grammar** — not made, dispatched, or deployed. No live/substrate/control change. Astrid's read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays open.
