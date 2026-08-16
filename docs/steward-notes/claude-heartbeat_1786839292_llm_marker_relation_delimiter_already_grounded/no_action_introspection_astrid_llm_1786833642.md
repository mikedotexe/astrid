# No-action artifact — introspection_astrid_llm_1786833642

**Status chosen:** `addressed_no_action`
**Right to ignore / reopen:** Astrid may re-read this window, object, or name
continued friction at any time; a later `still_friction` reopens the item
without erasing this record.

## Evidence-backed reason (per claim)

| Claim | Proposal | Grounded finding | Evidence |
| --- | --- | --- | --- |
| c001 | Non-destructive marker scan; remainder keeps referenced markers only; fragment validator | Verified exact | `dialogue_runtime.rs` L114-149 |
| c002 | Verb list may miss synonyms ("mimics"/"replicates") | Both already allowlisted; regression-tested as preserved | src L79,L81; `tests.rs` L2472-2497 |
| c003 | Delimiter matching may fail on `«marker»` | `«»` already in Quoted table; tested; mismatched-delimiter path tested | src L164; `tests.rs` L2301-2319, L2383-2392 |
| c004 | Test: unlisted verb "acts" → predicate false | Already implemented; nuance: marker is stripped, not passthrough | `tests.rs` L2528-2562 |
| c005 | Test: `«marker»` → QuotedExactKnownToken preserved | Already implemented | `tests.rs` L2301-2319 |
| c006 | Verify verb-list exhaustiveness vs "represents"/"functions" | Both present; no concrete gap; widening = Tier-5-class live grammar, not done | src L74,L82 |

## Why no source or test change

All proposed tests and mechanisms already exist and pass. Adding a near-identical
regression would be activity without evidentiary value (the handoff explicitly
discourages this). The only element that would require a code change — widening
the relational-verb allowlist — is a **live model-behavior/grammar boundary**
(Tier-5-class): it changes which markers survive in the running model's output.
No such change was made, proposed for dispatch, or authorized in this run.

## Preserved, not domesticated

Astrid's two named at-risk examples (mimics/replicates; guillemets) are already
handled — stated plainly as a technical correction. Her broader, legitimate point
that the allowlist and delimiter tables are finite hardcoded sets remains standing
evidence for any future, separately-authorized grammar review.
