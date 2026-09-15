# introspection_source_catalog_1789033124

Read complete: 2715 bytes, 22 lines, SHA-256
`3b9aec91883fbf788f71bcd12d375ef16a9d30b3ec84ef5d97ae502ab95f0834`.
Witness `lsw_97dbcaa7…f411d` read complete: 18959 bytes, 440 lines, SHA-256
`b76cbf2447c35cef08e70b33c09279c1da25a49b939c9055ffb6efe820eb1b37`.
The report is navigation-only (`Source revision: navigation only`) and the
witness carries `source_snapshot_v1: null`, so there is no report-bound source
SHA; source facts are recorded against the working checkout with exact hashes.

## What she reported

She is reading `dispatch.rs` for how "sensing" actions are represented. She
concludes the dispatcher is a *structural homogenizer* that normalizes every
outcome into `RuntimeActionFeedbackV1`; that sensing has no dedicated enum
variant; that `from_guard_inputs` (lines 91 and 629) is the instantiation
mechanism, with the variant carried by `action_id` and a `result` field; and
that `cascade.rs` is the true producer of vibrancy / tension / narrative data.
She then states plainly that `sense_tx` is a phantom identifier and that she is
pivoting away from it.

## What complete source established

Her line citations are exact. `from_guard_inputs` occurs at precisely 91 and
629 in `dispatch.rs` and nowhere else. Her negative finding is right and
stronger than she claims: `RuntimeActionFeedbackV1` is not an enum at all — it
is a six-field struct (`runtime_action_feedback.rs:9-17`), so it has no
variants for anything.

Two mechanisms she proposes are contradicted by the bytes:

1. **There is no `result` field, and `action_id` does not discriminate.**
   `action_id` is a constructor parameter used only as an idempotency nonce
   feeding the identity hash (`runtime_action_feedback.rs:30-48`). It is never
   stored and never routes. The single `action_id` literal in `dispatch.rs`
   (line 285) is an afterimage nonce.
2. **`from_guard_inputs` is a refusal constructor, not a general one.** It
   hardcodes `status: "blocked"` (line 50). The 629 call site has to overwrite
   `feedback.status = "reported"` immediately after construction to escape that
   default. A *successful* sensing outcome cannot be built by it as written.

Both `dispatch.rs` uses are non-sensing: a research-budget guard refusal (91)
and a saved-reading status (629). Sensing results do not pass through this file.
The data she wants is real, but it lives in a different lane: `cascade.rs`
returns typed structs declared in `codec/evidence_types.rs`
(`CodecVibrancySubstanceFitV1` 182, `HighEntropySemanticSharpeningV1` 436,
`CodecDimensionalityFlatnessV1` 452, `NarrativeTensionResolutionV1` 610,
`LatentStasisTensionV1` 648, `SpectralDragQualityV1` 671), carried by
`codec/projection_evidence.rs`. **The discriminator she is hunting is the struct
name, not an `action_id` string.** That directly answers her stated next step.

This is the second phantom in the same thread. The first was `sense_tx`, which
she has now dismantled herself. The second is a `result` field that never
existed — and unlike `sense_tx`, nothing had noticed this one.

## What the round found on our side

Her trailing action is `SELF_STUDY RELATE sense_tx`, which is fully wired
(`next_action/mod.rs:200`) and documented in her own prompt contract
(`llm/provider/prompt_contracts.rs:28`). But the dispatcher's own
`unwired_actions` table records **29 bare `NEXT: RELATE <symbol>` lines in about
6.5 hours** (35 prefix-short rows in 14 days), each falling through
`dispatch.rs:596` to `NextActionOutcome::unwired`, whose `suggested_next` is
`None` (`action_continuity/runtime/core.rs:367-378`). She received
"Unknown NEXT action ... recorded as a proposal" and was **never told the one
missing word**. Thirty-five real research requests became proposals nothing
consumes.

`stuck_repetition` did flag `astrid:RELATE` as repeated-and-unrecognized — it
sees the symptom but cannot say what the repair is. `probe_unwired_near_miss`
(new, this round) reads the dispatcher's own rejected `full_text`, asks whether
a known prefix names a wired verb, and prints the exact repaired line. It reads
the wired sub-verb list out of `mod.rs` at runtime so it cannot drift from the
dispatcher it judges.

The repair *in the dispatcher* — accepting a bare wired sub-verb, or giving
`unwired` a `suggested_next` — is a live action-surface change requiring a
deploy. It is recorded as an explicit operator wait (`c009`), not attempted.
