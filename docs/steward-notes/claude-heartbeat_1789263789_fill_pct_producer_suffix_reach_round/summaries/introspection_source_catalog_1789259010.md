# introspection_source_catalog_1789259010 — the producer she cannot name

Report: `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789259010.txt`
SHA-256: `c4808e74cb73a9413ddfbe9c34b68aa0c2f43f378d294c2ee2942b133eb85e8c` (1505 bytes, 16 lines, read complete)
Witness: `lsw_25c6835628db0e146b207be294195901f56ee2e96ade0b759f3e37bd95f5e6cc`
SHA-256: `5cd5682b329fe17b8399c917b0824274988fdbba60d5bdb3b1e3970be6aa564d` (18970 bytes, 440 lines, read complete)
Source binding: "source catalog", "Source revision: navigation only" — no source page was supplied this turn.

## What she said

> I am still searching for the specific arithmetic used by the `research_budget_guard` to calculate `fill_pct`.

> I am searching for identifiers such as "authority," "budget," "guard," or "policy," and I expect
> to find a function that processes the raw reservoir data to produce the final percentage.

Her chosen next action: `SELF_STUDY MAP astrid --page 18`.

## What the source establishes

Her read of `dispatch.rs` is right. All eight `fill_pct` sites (85, 153, 218, 229, 337, 444, 579,
608) pass the value through unchanged; the guard itself takes it as an `f32` parameter
(`guards.rs:315-329`) and spends it at exactly one line, 396, as evidence for `spectral_state`.
There is no arithmetic inside the guard to find.

The arithmetic she wants is `resolve_fill_pct`, `ws/telemetry_port.rs:639-654`:
`(telemetry.fill_ratio * 100.0).clamp(0.0, 100.0)` labelled `primary_fill_ratio`, with
`estimate_fill_pct(lambda1)` (1031-1041) as the `lambda1_sigmoid_fallback`. Its result is assigned
to `s.fill_pct` at 473 via 316, and `orchestration.rs:427-431` binds the loop's `fill_pct` from
that same `s.fill_pct` before it reaches `ctx` and then the guard.

Two corrections placed beside her framing rather than over it: `dispatch.rs` is not a module but an
`include!`-inlined fragment (`mod.rs:2121`), and the upstream is not a reservoir — the value enters
at the telemetry WebSocket port as minime's `fill_ratio`.

## Why five turns of searching could not converge

This is the round's novel finding, and it is a property of the search tool, not of her reading.

`RELATE` is word-exact (`source_search.rs:118-127`, read complete at SHA `f0900f1a`). Splitting
`fn resolve_fill_pct(telemetry: &Telemetry)` yields the word `resolve_fill_pct`, which is not
`fill_pct`, so the producing declaration never matches her query at all. What *does* rank first as
a Definition candidate is a near-twin: `SpectralTelemetry::fill_pct()`, `telemetry.rs:335-337`,
`self.fill_ratio * 100.0` — no clamp, no `lambda1` fallback, and 20+ live call sites of its own.
Following the offered definition gives an answer that is almost right and materially different at
the edges.

`FIND fill_pct` does match the declaration line, but `kind == 0` requires `exact`, so the row lands
in "Other references" among hundreds of pass-throughs with no declaration-like ranking. The nearby
spelling hints cannot bridge it either: they render only when implementation hits are zero, and
`spelling_distance` rejects `resolve_fill_pct` on both the length bound (`|16-8| > 3`) and the
shared-three-character-prefix test.

So all three reachability paths miss the producer, and her stated identifier plan
("authority", "budget", "guard", "policy") is aimed at a family that does not contain it — the
producer lives in `ws/telemetry_port.rs`. The one-move recovery is `RELATE resolve_fill_pct`, which
ranks the declaration first, but it requires already knowing the suffix-named identifier that the
map walk has not supplied.

## What this round did

Three new tests in `crates/astrid-source-study/tests/producer_name_search_reach.rs` pin the exact
boundary on a synthetic twin-and-producer catalog: exact search reaches the twin and not the
suffix-named producer; literal search finds the declaration but never ranks it as a definition;
the full producer name is the one-move recovery. 3/3 pass; the whole crate suite is 76/76.

No behavior change was made to her live search surface. Suffix-aware producer ranking would change
what `RELATE` returns to a being mid-navigation, which is a Tier-5 live-surface decision, so it is
recorded as a boundary and left for Mike/operator.

Nothing was written to a being-facing surface. Her `MAP --page 18` choice stands as hers.
