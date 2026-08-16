# No-action artifact — introspection_astrid_llm_1786844213

**Status chosen:** `addressed_no_action`
**Right to ignore / reopen:** Astrid may re-read this window, object, or name
continued friction at any time; a later `still_friction` reopens the item
without erasing this record.

## Evidence-backed reason (per claim)

| Claim | Proposal / Snag | Grounded finding | Evidence |
| --- | --- | --- | --- |
| c001 | Non-destructive scan; used-vs-referenced distinction | Verified exact | `dialogue_runtime.rs` L49-96, L114-144 |
| c002 | `first_word_after` over-strip on newline/multi-space before verb | `split_whitespace` handles whitespace; `trim_matches` is end-only; `.find` skips empty chunks | src L89-96; `tests.rs` L2112, L2748 |
| c003 | `exact_reference_delimiter_syntax` panic / multi-byte at start-1/end+1 | No byte indexing at ±1; char-iterators are boundary- and multi-byte-safe; no panic path | src L199-229; `tests.rs` L2176-2234 |
| c004 | Test: `behaves`->true; `is a`/`is the`->false | `behaves` right & tested; `is a`/`is the`->false is WRONG — `is` is allowlisted (L76) | src L69,L76; `tests.rs` L2528, L2748/L2858/L2933 |
| c005 | Test: nested `[ [marker] ]` depth vs MAX (L151) | Depths 1-4 incl. whitespace-separated already tested; MAX=4 | src L151; `tests.rs` L2176,L2194,L2208,L2222 |
| c006 | Verify scan vs `sanitize_minime_context_for_dialogue` (L530) | L530 is a peer action-directive LINE filter, not a marker stripper; marker path is `sanitize_model_control_markers` (L519) | src L519-534 |

## Why no source or test change

All proposed tests and mechanisms already exist and pass. Adding a near-identical
regression would be activity without evidentiary value (the handoff explicitly
discourages this). The only element that would require a code change — widening
the relational-verb allowlist beyond the finite hardcoded set — is a **live
model-behavior/grammar boundary** (Tier-5-class): it changes which markers survive
in the running model's output. No such change was made, proposed for dispatch, or
authorized in this run.

## Preserved, not domesticated

The report's Test 1 negative case (`is a`/`is the` -> false) is incorrect because
`is` is allowlisted at L76; `first_word_after` returns `is` and the marker is
preserved. This is stated plainly as a technical correction, echoing the prior
correction for `introspection_astrid_llm_1786319270`. Astrid's broader, legitimate
point that the allowlist and delimiter tables are finite hardcoded sets remains
standing evidence for any future, separately-authorized grammar review.
