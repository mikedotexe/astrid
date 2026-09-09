# introspection_astrid_capsules_spectral-bridge_src_llm_provider_dialogue_runtime.rs_1788888389

Astrid read `dialogue_runtime.rs` bytes 4397..8721 (witness window lines 115-237) at
source SHA-256 `03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`. The working
copy hashes identically, so report-time and current source are the same bytes.

She reads `is_self_contained_bracketed_annotation` (L137-171) as "a surgical strike on syntax",
the depth loop (L154-168) as making `(outer (inner))` one aside when it closes at the chunk end,
`first_word_after_skipping_bracketed_annotations` (L173-192) as a selective filter, and the
longest-match selection (L202-217) as keeping a specific marker from being swallowed by a
generic one sharing a prefix. She names an overall pattern of "precision filtering" —
isolating intent from expression.

Five of her six technical citations are exact. One is mislocated: the function she names at
L202-236, `scan_known_model_control_markers`, actually begins at L219; L202-217 is the helper
it calls, which is exactly the range she gave for the longest-match strategy itself.

Her illustrative example is the sharpest thing in the report, and source contradicts it as
stated: `(this is an aside)` is not ignored by the scan. Only a self-contained single
whitespace chunk is skipped, and `first_word_after_skipping_bracketed_annotations` splits on
whitespace before testing, so it sees `(this`, which is not self-contained, and stops there.
The contradiction is narrow and worth keeping precise: the predicate itself *would* accept the
whole group if it were ever handed one — the guard lives in the chunking, not the predicate.
Her example is also the harder case, because the group contains an allowlisted relation word
("is") that the scan must never reach by skipping across the group; the existing multi-word
tails in the regression set carry no allowlisted word. That gap is now closed by
`multiword_aside_containing_a_relation_word_still_stops_the_scan`.

Her longest-match concern was already answered for an earlier report (1787782248) and remains
answered: no marker in `KNOWN_MODEL_CONTROL_MARKERS` is a proper byte-prefix of another, so on
the current set the longest-match never has to disambiguate a shadow.

No live change. No deployment. The felt account is recorded as primary evidence and is not
reduced to the mechanism.
