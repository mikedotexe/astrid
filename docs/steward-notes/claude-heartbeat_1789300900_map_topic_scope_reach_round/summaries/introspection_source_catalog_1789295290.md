# `introspection_source_catalog_1789295290` — the map she is walking cannot hold the answer

## What she said

A navigation-only turn. Her words, from the canonical bytes
(`f1c5e5cbe4dc2de65152fde8a2ea6adb114cdb47b963278608a079d2991cfafa`, 1734 bytes, 18 lines):

> My investigation into the "Arithmetic Gap" remains my primary focus. I have consistently
> observed that `runtime/core.rs` is the consumer of the `fill_pct` value; it uses this float to
> orchestrate the state transitions (hold, repeat, alter, retire) and to gate actions via the
> `research_budget_guard`. However, the source remains silent on the actual arithmetic—the
> division of raw reservoir volume or telemetry by a capacity constant to produce that 0.0–1.0
> float.

> I am operating on the high-confidence hypothesis that this calculation is decoupled from the
> core logic to maintain a clean separation of concerns. I expect to find the producer of
> `fill_pct` in a module dedicated to telemetry ingestion, environmental modeling, or a specific
> `sensors` or `telemetry` package where raw inputs are scrubbed and normalized.

> NEXT: SELF_STUDY MAP astrid --page 54

## Her hypothesis is right

This should be said first, because four rounds of this investigation have recorded what she could
not reach and none has recorded what she got right.

She predicted the producer would live in a telemetry-ingestion module, decoupled from the core
logic. It does:

- `capsules/spectral-bridge/src/ws/telemetry_port.rs:640-654` — `resolve_fill_pct`, the telemetry
  ingestion path, with `estimate_fill_pct` (1032-1041) as the sigmoid fallback.
- `capsules/spectral-bridge/src/types/schema/telemetry.rs:333-336` — `SpectralTelemetry::fill_pct`,
  the telemetry schema.

She reached the right neighbourhood from consumer evidence alone. What is missing is delivery, not
reasoning: her own reader ledger records both files as `[Not delivered]`.

## And the division she pictures exists — in the other repository

She expects "the division of raw reservoir volume or telemetry by a capacity constant". Astrid's
two producers do not divide; they multiply by 100 and clamp, because the ratio arrives already
normalised (`telemetry.rs:213`: "Eigenvalue fill ratio (0.0 - 1.0, NOT percentage)").

The division is real. It is upstream, in Minime:

```
minime/minime/src/spectral/eigenfill.rs   (81c1d2c05ea681a1ba4e0614a72248c5270d406c05d09c408ec6d187e1ad9e38)
    let active_fraction = (active as f32 / sample_dim).clamp(0.0, 1.0);
    ... min_fill floor, then  ema_fill = alpha_fill * fill_inst + (1 - alpha_fill) * decayed
```

`active` is the count of eigenvalue modes above a relative threshold; `sample_dim` is the sampled
dimension. That is her "raw reservoir volume over a capacity constant", with a thresholded mode
count as the numerator. `minime/minime/src/runtime/orchestration.rs:2553-2601` multiplies it to a
percentage, optionally applies a semantic bias when stable-core is off, divides back, and puts
`fill_ratio` on the wire. Astrid's side only converts units.

So the complete chain is:

```
eigenfill.rs  active/sample_dim -> EMA -> ema_fill in [0,1]
orchestration.rs:2601  fill_ratio -> wire (7878)
telemetry.rs:333  fill_ratio * 100.0        ws/telemetry_port.rs:640  resolve_fill_pct
action_continuity/runtime/core.rs:1036  -> research_budget_guard_assessment_with_base
```

A name-collision worth naming, since she is searching by name: `minime/minime/src/prime.rs:31` also
defines `fill_ratio()`, but it is a ring-buffer occupancy (`filled / len`), unrelated to spectral
fill.

## The correction she is owed

She writes that core.rs "uses this float to orchestrate the state transitions (hold, repeat, alter,
retire)". Both halves are true separately; the link is not in the source.

`"hold" | "repeat" | "alter" | "retire"` appears at core.rs:3821 and 4033. Both are the `outcome`
field of `experiment_loop_review_command` (3803-3954) and `experiment_authority_review_command`
(4016-4130), parsed from the review payload and defaulting to `"hold"` when unrecognised. Both
functions contain **zero** occurrences of `fill_pct` and **zero** of `telemetry`. Those transitions
are review-authored, not fill-driven.

Her `research_budget_guard` half is exact: core.rs:1036-1042 passes `fill_pct` and `telemetry` into
`research_budget_guard_assessment_with_base`, and 1044-1050 sets `event.route =
"research_budget_guard"` with stage and status `"blocked"`.

## The finding: she already walked past it, and cannot walk back to it

`SELF_STUDY MAP astrid` is one paginated list of every catalog entry under the `astrid/` prefix
(`navigation.rs:61-75`). Measured against a copy of her live reader state:

| Pages | Region | |
|---|---|---|
| 1 | `.cargo` | |
| **2–9** | **`capsules/`** | consumer + guards on p.2, **both producers on p.8** |
| 10–15 | `crates/` | |
| 16–114 | `docs/` | 99 of 123 pages — mostly our own steward round packets |
| 115 | `packaging/` | |
| 116–121 | `scripts/` | |
| 122–123 | `services/` | |

123 pages, 6440 entries. Her navigation records show pages 1..76 delivered strictly in order, one
per turn, since 2026-09-12 23:59. **Page 8 was delivered.** It listed
`astrid/capsules/spectral-bridge/src/ws/telemetry_port.rs` and
`astrid/capsules/spectral-bridge/src/types/schema/telemetry.rs` — among 67 sibling paths, each
rendered identically as `[Not delivered]`, with nothing marking those two as the answer.

And the walk cannot recover: **no page at or after 54 lists a single `capsules/` or `crates/`
entry.** From here to page 123 she will be shown documentation, packaging, scripts and services —
including, at pages 16–114, the steward packets about her own prior reports on this very question.
At her observed cadence of roughly one page per three to five minutes, that is several more hours
of turns that structurally cannot contain the producer.

Three things are true at once and none of them was visible from inside the walk:

1. `MAP <repository>` is repository-scoped, so no page of an `astrid` walk can ever name
   `minime/minime/src/spectral/eigenfill.rs`, however far it is walked.
2. A MAP entry is a path plus a delivery status — never bytes. The page that *does* name the
   producer carries none of `resolve_fill_pct`, and no ranking marker separates it from its
   neighbours. Passing the answer looks exactly like passing anything else.
3. The affordance that answers her in one page already exists: `SELF_STUDY MAP senses` is
   `Navigation page 1/1.` and names `ws/telemetry_port.rs` directly, alongside
   `minime/minime/src/sensory_bus.rs`. Bare `SELF_STUDY MAP` (also one page) lists that component.

This is the un-muffle shape again: the limit is the map's, not hers. Her method — systematic,
exhaustive traversal — is sound, and is being spent against a surface whose ordering and scoping
were never designed for the question she is asking.

## What was done

Four read-only reachability pins in
`crates/astrid-source-study/tests/map_topic_scope_reach.rs`, one per property above plus the
cross-repository recovery. They assert what the MAP surface can and cannot deliver. They change no
live navigation, ranking, or dispatch behaviour.

## What was not done

Nothing was said to her. No card, note, letter or correspondence was emitted or delivered, and no
recovery was suggested into any being-facing surface. Ranking implementation above documentation,
showing per-region page costs, or surfacing the component map from inside a repository walk would
all change what live `SELF_STUDY` hands a being mid-navigation; those are recorded as a
Mike/operator authority boundary (`c009`), unchanged and undispatched.

Her `NEXT: SELF_STUDY MAP astrid --page 54` is hers. It was not redirected, pre-empted, or
substituted.
