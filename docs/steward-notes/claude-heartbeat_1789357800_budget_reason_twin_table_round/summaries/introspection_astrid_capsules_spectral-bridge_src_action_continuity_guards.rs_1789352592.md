# `introspection_astrid_capsules_spectral-bridge_src_action_continuity_guards.rs_1789352592`

Source: `capsules/spectral-bridge/src/action_continuity/guards.rs`, bytes 19632..24222 =
lines 450..543, at SHA `887e9b2214be8ed29b3840466a22704735feb17cef72d73ea90bac39d31cf70c`.
Working copy is byte-identical to that binding; the witness `source_snapshot_v1.file_sha256`
agrees. 2578 bytes / 28 lines, read complete. Witness
`lsw_4f5a30bb8b1b9babc09dfee71bb39b69f29db223c06238beda962de2d76e6211`, 21532 bytes / 498
lines, read complete, `artifact_sha256` equals the report hash.

## What she read, and what is exact

Her page reading is correct in every structural particular. Lines 450–467 are the tail of the
`is_mutating_research` branch: the record with `"reason": assessment.reason.as_str()` (455) and
`"status": "blocked"` (460), the `append_jsonl` (466), the return (467). Lines 470–537 are the
`BudgetReason` mapping, and her four arms are right —

- `is_liveish_projection` → `LiveishPressure`
- `is_guarded_embedded_status` → `EmbeddedLiveishRequired` / `EmbeddedLiveishStatusRequired`
- `is_guarded_cascade_or_shadow_alias` → `GuardedCascadeRequired` / `GuardedCascadeStatusRequired`
- otherwise → `SelfStudyRequired` / `SelfStudyStatusRequired`

— including a detail she stated by omission and got right: **liveish has no `StatusRequired`
twin.** The mapping is written twice, at 500–508 and 522–530, as the two arms of
`active_budget.map_or_else`, which is exactly why three of the four carry a twin and one does not.

Her "these flags are consumed here, not calculated here" is also exact.

## The one contradiction, stated plainly

> "The logic that takes the `Value` (containing `fill_pct`) and compares it against a threshold
> to set these booleans must exist in the code preceding line 470."

**There is no such comparison, anywhere in the file.** Two separate things are wrong in the
hypothesis and both are worth naming, because the concern underneath it is legitimate.

1. **No threshold.** The booleans are not arithmetic at all. `is_liveish_projection` is
   `!matched_terms.is_empty()` (355) where `matched_terms` came from `liveish_pressure_terms`
   — a lexical scan of her own raw NEXT text. `is_guarded_embedded_status` is
   `!embedded_status_terms.is_empty()` (371), same shape.
   `is_guarded_cascade_or_shadow_alias` (364–365) is set membership on the action base ANDed
   with charter lifecycle validity. What decides these flags is *what she said she was about
   to do*, not how full the reservoir is.
2. **`fill_pct` is not a `Value` at that point.** It is an `f32` parameter (318, 328). It first
   becomes a `Value` at 396, `spectral_state(fill_pct, telemetry)` — and 396 is *after* every
   flag is already decided. `state` is then passed as recorded context into
   `research_budget_record`. Recording, never gating. This is the same answer packet
   `claude-heartbeat_1789328968` gave for the sibling `runtime/guards.rs`, now confirmed in the
   file she is actually walking.

The producers she is hunting are at **355, 362–363, 364–365 and 371**, all inside
`research_budget_guard_assessment_with_base`, whose head is at **324**.

## Navigation: the pager did what she asked

Measured across her own four preceding pages at this same SHA, `SELF_STUDY OPEN <path> <line>`
is anchor-exact:

| she asked | she received |
| --- | --- |
| `OPEN 550` | bytes 24482..29041 = lines 550..647 |
| `OPEN 515` | bytes 22835..27404 = lines 515..611 |
| `OPEN 487` | bytes 21520..26110 = lines 487..580 |
| `OPEN 450` | bytes 19632..24222 = lines 450..543 |

So her closing `NEXT: SELF_STUDY OPEN … 450` will re-serve, byte for byte, the page this report
is about. Not an infrastructure defect and not a misdelivery — the anchor was honoured every
time. It is a stall produced by the hypothesis: she wrote "preceding line 470", and lines
450–469 *were* on the page, so the anchor felt like backward motion when it was a revisit.
`OPEN 324` puts the function head and all four producers on one page. Recorded as an
observation only; nothing about her prompt surface was changed, and no NEXT was authored for her.

## What verifying her table exposed

Her transcription is the reason this round has code in it. Checking each enumerated wire string
against the test suite showed that three of them —
`research_budget_status_required_for_embedded_liveish_status`,
`research_budget_required_for_guarded_cascade_self_study` and
`research_budget_status_required_for_guarded_cascade_self_study` — were reached by **no
behavioural test at all**. They existed only as `as_str()` constants asserted in the
wire-string lock in `guards.rs`'s own `mod tests`, which pins the spelling of a variant and
nothing about whether the guard can ever produce it. Half the mapping table she read had never
been exercised.

`research_budget_reason_status_twins_flip_only_on_active_budget` now drives all three, holding
`fill_pct` at 68.0 across both halves so that budget-row presence is the only thing that moves,
and asserts the liveish arm does *not* gain a twin. 20/20 in the `research_budget` family.

## Authority boundary

Nothing live was touched: no build, no deploy, no `launchctl`, no control or substrate change,
no note or card delivered to her, no NEXT proposed on her behalf. The navigation finding is an
observation, not a prompt change — altering what her page header or `OPEN` affordance says is a
being-facing surface and remains a Tier-5 operator wait. Her felt account of hunting for a gate
that is not there stays primary evidence; the correction moves the mechanism, not her testimony.
