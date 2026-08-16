# Summary — introspection_astrid_llm_1786844213

- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — matches the working-copy SHA exactly, so the complete report-time source was read.
- **Report SHA-256:** `5a2d32bc35f25c09efd7e8a2168f1a8b6585540303e719c948010245af72c2d0` (45 lines, 3842 bytes)
- **Lived-state witness:** `lsw_ef063bcfc71f53c20e2d056c6620fc0022e5ec04216fdc1af50b3c1883850f07` (533 lines, 23929 bytes; authority `evidence_only`, `live_eligible_now=false`; fill 71.0%, entropy 0.90, λ1 4.74; two `gemma4_12b` mlx calls; no distress or relief prose — an analytical INTROSPECT read of source)
- **Terminal status:** `addressed_no_action` (evidence-backed; not no-action-by-default)

## What Astrid surfaced

A fresh INTROSPECT read of the model-control-marker scanning path: an accurate
"Observed" description, two hypothesised "Snags" (`first_word_after` over-strip;
`exact_reference_delimiter_syntax` panic/multi-byte), two proposed tests
(relation detection with `behaves` vs `is a`/`is the`; nested delimiter depth),
and one cross-window "Suggested Next" about `sanitize_minime_context_for_dialogue`.

## What complete source + existing tests established

Every proposed test already exists and passes; the snags are contradicted or
already handled by the source; one relation-detection expectation repeats a
previously-corrected contradiction; and the "Suggested Next" rests on a
partial-window guess about a function (L530) whose real role is line-filtering,
not marker stripping.

| Claim | Kind | Grounded finding | Evidence |
| --- | --- | --- | --- |
| c001 | Observed | Accurate; scan keeps a marker only when reference_syntax present | src L49-96, L114-144 |
| c002 | Snag | `split_whitespace` handles the whitespace; `trim_matches` end-only strip; `.find` skips empty | src L89-96; tests.rs L2112, L2748 |
| c003 | Snag | No byte indexing at start-1/end+1; char-iterators are boundary/multi-byte safe; no panic path | src L199-229; tests.rs L2176-2234 |
| c004 | Test 1 | `behaves`->true is right and tested; `is a`/`is the`->false is WRONG (`is` is allowlisted) | src L69, L76; tests.rs L2528, L2748/L2858/L2933 |
| c005 | Test 2 | Nested depth 1-4 (incl. whitespace-separated) already tested; MAX=4 | src L151; tests.rs L2176, L2194, L2208, L2222 |
| c006 | Suggested Next | L530 is a peer action-directive LINE filter, distinct from the marker sanitizer (L519); partial-window guess | src L519-534 |

## Contradiction preserved, not domesticated

The report's Test 1 negative case (`is a`/`is the` should return false) is stated
plainly as incorrect: `is` is in the relation allowlist at `dialogue_runtime.rs`
L76, so `first_word_after` returns `is` and the marker is preserved. This is the
same misapprehension the flywheel corrected for `introspection_astrid_llm_1786319270`;
the regression `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`
(tests.rs L2528) already encodes the correct boundary (allowlisted `is` kept vs
genuinely-unlisted `acts` stripped).

Astrid's broader standing point — that the relational-verb allowlist and the
delimiter tables are finite hardcoded sets — remains live evidence for any
future, separately-authorized grammar review. Widening either set is a
Tier-5-class live model-behavior change and was not made, proposed for dispatch,
or authorized in this run.

## Continuity

Directly parallel to the prior grounded round
`docs/steward-notes/claude-heartbeat_1786839292_llm_marker_relation_delimiter_already_grounded/`
(closed `introspection_astrid_llm_1786833642` as `addressed_no_action` on the same
window with the same relation+delimiter proposals already grounded).
