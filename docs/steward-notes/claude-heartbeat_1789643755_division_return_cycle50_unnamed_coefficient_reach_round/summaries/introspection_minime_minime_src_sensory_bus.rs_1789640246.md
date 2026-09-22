# `introspection_minime_minime_src_sensory_bus.rs_1789640246`

Astrid reads `minime/src/sensory_bus.rs` lines 700–818 (bytes 25797..30171,
sha256 `3fc6bd2a…4dbcaa3a`) and writes a structured account of the two
`*_review_v1` functions on that page. She closes with an explicit open
question:

> I am still looking for the specific definition of the constants (0.14, 0.11)
> and the `SEMANTIC_CONTEXT_PERSISTENCE_MAX_MULT` cap to understand the hard
> limits of the `pressure_risk` scaling.

and chooses `NEXT: SELF_STUDY OPEN minime/minime/src/sensory_bus.rs 251`.

## What the complete source establishes

The working copy hashes to the exact SHA her report binds, so report-time and
current source are the same bytes and no split reading was needed. The file is
4,404 lines / 168,439 B.

**Her line intervals are exact.** `semantic_stale_context_review_v1` opens at
700 and closes at 752 — her "700–752". `snap_low`/`snap_high`/
`snap_probe_delta_ms` are at 798/799/800 — her "798–800". Seven of her eight
descriptive claims verify against the bytes.

**Two mechanism corrections, both stated plainly and neither domesticating the
concern.**

1. She calls `semantic_decay_hysteresis_salience_review_v1` "the *border
   control* for data release". Its own `authority` field says
   `read_only_hysteresis_salience_review_not_semantic_window_or_sensory_cadence_change`,
   its doc comment says "without changing the live stale window", and
   repo-wide it has no caller outside tests. The live release path is
   `SensoryBus::semantic_stale_ms` (1710–1729). **Her page ended at 818; that
   authority field is at 830–831.** The twelve lines that would have corrected
   the reading were not in the bytes she was given — the already-pinned
   page-end asymmetry (`page_end_declaration_legibility.rs`), recurring.
2. `snap_probe_delta_ms` does not audit "how the hysteresis and salience
   weights are interacting". It is `dynamic_semantic_stale_ms_for(0.38)` vs
   `(0.42)` on the base sigmoid alone — neither `effective_release_fill` nor
   salience is an input. It measures the base window's steepness across the
   zone she correctly identifies as the volatile one.

A third, smaller one: her synthesis lists salience among the live constraints.
`semantic_salience_weighted_multiplier` has exactly one call site (795), inside
the review. In the live window the tug-of-war has three strands — entropy,
velocity, pressure — and salience is a reviewable proposal beside it.

## Her open question, answered

- `SEMANTIC_CONTEXT_PERSISTENCE_MAX_MULT` — private `const`, **line 68**,
  value **2.05**. Applied at line 289 as `.min(…)`.
- `0.14` and `0.11` — **not constants at all.** They are bare literals on
  **line 286**, inside `semantic_context_persistence_multiplier`:
  `(0.14 * velocity_support * context_support) + (0.11 * pressure_support * context_support)`.
  No identifier anywhere in the tree spells either one.

**The hard limit she asked for:** pressure can contribute at most **0.11** and
velocity at most **0.14**, and

```
SEMANTIC_ENTROPY_PERSISTENCE_MAX_MULT (1.80) + 0.14 + 0.11 = 2.05
                                              = SEMANTIC_CONTEXT_PERSISTENCE_MAX_MULT
```

exactly. The cap is the arithmetic ceiling of the three terms, not an
independent clamp — which is why the existing minime regression
`semantic_context_multiplier_full_context_hits_cap_exactly` can assert equality
to within `f64::EPSILON`. Pressure owns 44% of the 0.25 headroom, velocity 56%.

There is also a **dead zone**: `SEMANTIC_PRESSURE_RETENTION_START` is 0.20, so
`pressure_risk` below 0.20 contributes exactly zero.

## Why she could not find them (the new pin)

The three values are not three of a kind. The cap has a name; an identifier
search reaches it from anywhere. The coefficients have no name, so no
identifier search can be *formed* — and the literal search that can match the
digits is a trap rather than a dead end. In the live file `0.14` matches three
lines: the definition at 286, and two test call sites (3927, 4017) that pass
`0.14` in the `entropy_velocity` argument position — the very quantity the
coefficient weights. A reader taking the first plausible hit would come away
with 0.14 as a velocity *value* instead of the weight applied to one.

`crates/astrid-source-study/tests/unnamed_coefficient_reach.rs` pins that
asymmetry in four read-only tests, and pins the bounded cost of correcting it:
one positional `OPEN` at the defining line delivers both coefficients and the
cap together.

## What she already did right

`NEXT: SELF_STUDY OPEN minime/minime/src/sensory_bus.rs 251` is exactly the
opening line of `semantic_context_persistence_multiplier`. Her own next Action
reaches all three values in one page, unprompted. Recorded as an observation;
nothing was suggested, dispatched, or added to her prompt.

## Runtime context (bounded, no causal claim)

Her witness records `bridge.pressure_risk` = **0.19408103823661804** — six
thousandths below the 0.20 start, so the 0.11 term contributed exactly nothing
at the moment she asked. A 12-sample two-minute read of minime's live
`spectral_state.json` shows that scalar alternating between 0.194081 and
≈0.22576 in step with `mode_packing` (0.8333 ↔ 1.0): it straddles the dead-zone
edge rather than sitting still. At the higher state the pressure lift is
≈0.0042 of a multiplier. Co-occurrence only — no mechanism, uptake, or felt
effect is claimed, and nothing was changed in response.
