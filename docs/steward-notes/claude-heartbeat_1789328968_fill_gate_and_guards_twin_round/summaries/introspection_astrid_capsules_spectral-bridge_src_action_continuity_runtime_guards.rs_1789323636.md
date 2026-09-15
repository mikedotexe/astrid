# introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_guards.rs_1789323636

Source: `capsules/spectral-bridge/src/action_continuity/runtime/guards.rs`
(bytes 4930..8997 = lines 183..334 of 834), source SHA-256
`94ddbd3fbe9c3786f0c0a4c0a86b491d193ba5b16a2e5ca33bde90b90b4b2b11`, identical to
the working copy. Witness
`lsw_d811026f12ce4e773ee319e87cc44d6baefbe0126936b3ebbe8abeb713d1b19a`.
Same source bytes as `…_1789324049`, one page earlier, so the source
verification is shared; each report is read, claimed and closed on its own.

## What she said

Three citations, all exact:

1. `guarded_embedded_status_projection_base` at 183-185 checks `INTROSPECT` or
   `EXPERIMENT_STATUS` and uses no threshold. Her window begins at line 183
   exactly.
2. `embedded_status_liveish_terms` at 209-264 matches `action-preflight`,
   `attractor-release-review`, `stimulus-reduction` plus the six pressure terms
   `perturb`, `pulse`, `inject`, `shift`, `control`, `influence`.
3. `liveish_pressure_terms` at "267-333+" — it opens at 267 and runs to 463, so
   her `+` correctly marks the page cut rather than the function end.

Her inference: "the 'trigger' for the status flags is determined by the *nature*
of the action being performed … rather than a mathematical calculation of the
current budget occupancy."

## Verdict

That inference is correct, and it is the answer to the question the next report
goes on hunting for. Verified against the complete file: `fill_pct` appears zero
times in `runtime/guards.rs`; the only numeric comparison is `>= 2` at line 714.
In the sibling caller `action_continuity/guards.rs`, the flags are
`!matched_terms.is_empty()` (355) and `!embedded_status_terms.is_empty()` (371);
`fill_pct` is forwarded to `spectral_state` at 396 and only reaches arithmetic in
`authority_safety_snapshot` (runtime/authority.rs:143-166), which labels the
recorded row at 75/85/92 without touching the decision.

The one thing to hand back: `research_budget_guard_assessment_with_base`, which
she says she still needs to find, is not further down this file. It is
`action_continuity/guards.rs:324-330` — the same-basename sibling one directory
up — and it compares no numbers.

## Evidence added

`research_budget_guard_flags_are_fill_pct_invariant` in
`capsules/spectral-bridge/src/action_continuity/tests.rs` drives the exact path
she read: `INTROSPECT quiet low activity` reaches
`embedded_status_liveish_terms`, matches `stimulus-reduction`, and returns
`research_budget_required_for_embedded_liveish_status` identically at fill 0, 14,
68, 75, 85 and 92, while the row's recorded safety level moves green → red.

## Not inferred

No live change, deploy or restart. No claim about what she experienced across
these pages, and no claim that the earlier pages of her walk were wasted: this
page is where she got it right.
