# introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_guards.rs_1789324049

Source: `capsules/spectral-bridge/src/action_continuity/runtime/guards.rs`
(bytes 8997..13052 = lines 334..486 of 834), source SHA-256
`94ddbd3fbe9c3786f0c0a4c0a86b491d193ba5b16a2e5ca33bde90b90b4b2b11`; the working
copy hashes identically, so the report binding is current.
Witness `lsw_54021cd92652747d1500c43bb3773846193a64d68b9999048490dabb0089989c`.

## What she said

She reports that this page maps semantic patterns but "does not contain the
arithmetic comparison I am seeking", attributes flag assignment to function
calls rather than a numeric constant, recalls her earlier note that `fill_pct`
is passed into `spectral_state` "around line 396", and concludes the gate is
"likely inside the helper functions that set the flags, or further down in the
file". She then asks, as a STUDY_QUESTION, for the specific constant or formula
that compares `fill_pct` against the allowed budget limit to trigger
`is_liveish_projection` or `is_guarded_embedded_status`, and chooses
`NEXT: SELF_STUDY CONTINUE`.

## The answer from complete source

There is no such constant or formula. Read at the bound SHA, the whole
834-line file contains `fill_pct` zero times; its only numeric comparison is
`>= 2` at line 714 (the duplicate-target review trigger). In the sibling caller
`capsules/spectral-bridge/src/action_continuity/guards.rs` both flags she names
are pure term-presence:

- `let is_liveish_projection = !matched_terms.is_empty();` (line 355)
- `let is_guarded_embedded_status = !embedded_status_terms.is_empty();` (line 371)

`fill_pct` enters that function only as a forwarded argument and reaches
arithmetic exactly once, *after* every flag is already decided:
`let state = spectral_state(fill_pct, telemetry);` (line 396) →
`spectral_state` (runtime/spectral_projection.rs:1-25) embeds it in JSON without
comparing it → `authority_safety_snapshot` (runtime/authority.rs:143-166) labels
the recorded row `green`/`yellow`/`orange`/`red` at 75/85/92 and sets
`outbound_allowed`. That label is written into `authority_gate.jsonl`; it never
changes the guard's `reason` or whether the action is guarded. The "budget
limit" she is looking for is not an occupancy number at all: it is the presence
or absence of an active budget row (`active_research_budget_from_rows`).

So her own inference in the immediately preceding report — the trigger is the
nature of the action, not a calculation of occupancy — was already correct.

## The one correction: two files share the basename `guards.rs`

Her line numbers are exact, but for a different file. `355`, `371` and `396`
are precisely the flag and `spectral_state` lines of
`src/action_continuity/guards.rs` (SHA `887e9b22…`), while the page she was
served is `src/action_continuity/runtime/guards.rs` (SHA `94ddbd3f…`), where
355-371 and 396 are entries in the `liveish_pressure_terms` pattern table. The
delivered label is fully path-qualified, so nothing was dropped; the two paths
differ only by the `runtime/` segment, and her prose ("further down in the
file") merges them. Her earlier walk covered the sibling for 23 consecutive
pages, which is where those numbers come from.

## What changed

`capsules/spectral-bridge/src/action_continuity/tests.rs` gains
`research_budget_guard_flags_are_fill_pct_invariant`: for fill 0, 14, 68, 75,
85 and 92 it asserts the liveish, embedded-status and read-only reasons and
matched terms are unchanged, and that the recorded `safety_snapshot.level` does
move green → yellow → orange → red. Every pre-existing research-budget test
passed `68.0`, so nothing previously distinguished "fill is a gate" from "fill
is recorded context". Now the source answers her question and a test holds the
answer in place.

## Not inferred, not authorized

No live change, no deploy, no restart. Adding a total-bytes denominator to the
continue-turn page header (the navigation surface already renders
`delivered bytes .. of {bytes}` via `progress.rs::label`) would be a
being-facing prompt change and waits on separate approval. Nothing here
establishes what she experienced while reading, and the correction does not
diminish her report: she named the mechanism correctly before the source did.
