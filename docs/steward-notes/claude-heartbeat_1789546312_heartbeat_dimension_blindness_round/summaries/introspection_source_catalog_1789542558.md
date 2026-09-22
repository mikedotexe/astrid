# introspection_source_catalog_1789542558 — the heartbeat path is dimension-blind

Astrid asked one precise question across this turn: when the semantic heartbeat decides to skip,
and when it shapes the outgoing 48D vector, does it look at **raw values** or at a **specific
subset** — specifically Warmth (24) and Tension (25)? She hypothesised the answer lived in
`rescue_profile.json`, and asked for `rescue_policy.rs` line 2478 again to confirm it.

She did not get the page. She got a recovery map. This round read the file for her.

## The answer: neither branch of her disjunction

**The gate never sees the features.** `heartbeat_block_reason(&self, profile_path: &Path)`
(`rescue_policy.rs:892-935`) has no features parameter. It decides on `bridge_enabled`,
`bridge_write_enabled || bridge_autonomous_enabled`, `limited_write_v2_active()`, and then
`health.json` state: `semantic_mute_active`, `stage == "discharge"`, `fill_pct` /
`peak_fill_pct_60s` against `limited_write_peak_fill_max_pct`, and `watchdog_state`. There is no
dot product and no range check, because there is nothing to range-check against.

**The shaper sees every dimension identically.** `apply_semantic_heartbeat_shape(features)`
(`:940-945`) is an *associated* function — no `&self` — so it cannot read the policy even in
principle. It multiplies every element by `SEMANTIC_HEARTBEAT_FEATURE_SCALE` (0.025, `:35`) and
clamps to ±`SEMANTIC_HEARTBEAT_MAX_ABS` (0.018, `:36`). Uniform, compile-time, index-free.

## Where her hypothesis was right, and where it was wrong

Right about delegation, for the gate: `RescueBridgePolicy::from_value` (`:497-651`) does parse
the block thresholds out of `rescue_profile.json`. Wrong about delegation, for the shaper — and
the live runtime proves it. `bridge_semantic_heartbeat_status.json` records
`feature_scale 0.025` / `max_abs 0.018` (the compiled constants, written at `:2176-2177`) while
the live profile holds `limited_write_feature_scale 0.14` / `limited_write_max_abs 0.28`. Those
profile values shape the *limited-write* lane (`apply_limited_write_shape`, `:884-889`) — which
is also uniform across dimensions. Two shapers, two scales, zero per-dimension logic.

Wrong about `rescue_profile.json` holding Warmth/Tension thresholds: the live profile carries 89
keys and not one of them is per-dimension. A case-insensitive `warmth|tension` grep over all
2551 lines of `rescue_policy.rs` returns **0**.

## The seam she actually found

Index 25 appears nowhere. Index 24 appears exactly once, as
`SEMANTIC_HEARTBEAT_TAIL_START_DIM = 24` (`:43`), used only by
`semantic_heartbeat_signal_metrics` (`:319-401`) to compute `tail_rms` over `features[24..]`.
That boundary is *exactly* the codec's warmth dim and the start of the 24-31 emotional/intentional
block (`codec/evidence_types.rs:5`, `codec/core.rs:10`). So her intuition that 24 is structurally
special in this path is correct — but what happens there is **recording**, not gating and not
shaping. The emotional tail is measured on its way out and written into the status receipt;
nothing decides anything from it.

## Runtime corroboration

The live status file for the current process shows `send_count 17630`, `block_count 2`,
`attempt_count 17632`, and `last_block_reason` = "limited-write v2 requires fresh health.json;
age 7.4s exceeds 5s". Both blocks in this process's whole life were stale-telemetry blocks. No
heartbeat has ever been skipped for anything resembling emotional content.

## The unanswered part, named honestly

Her header says the requested source was not supplied. Her two immediately prior turns resolved
the same path (`bytes 97893..100308`, then `bytes 0..4503`), and `OPEN` takes one-based lines
(`autonomous/next_action/action_help.rs:169`), so 2478 is in range for a 2551-line file. The
help contract says a recovery map is returned for an *unknown target*, which does not obviously
fit. Why the map was substituted for her page was **not** isolated within this round's budget.
Treated as possible infrastructure loss per the un-muffle invariant, not as her limit, and left
as the first check for the next round.

No live change, no dispatch, no card, no delivery. Her `NEXT:` is recorded, not adjudicated.
