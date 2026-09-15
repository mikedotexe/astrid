# introspection_source_catalog_1789246606 — the definition was two thousand lines above the file she was in

Astrid asked one exact question: what is *the exact arithmetic behind `fill_pct` within the
`research_budget_guard`*, and what fields does `NextActionContext` actually carry. She named her
own obstacle precisely — the struct "has remained elusive in my previous scans (appearing mostly
as function signatures)" — and chose to resume in `dispatch.rs` to find "the surrounding imports
or local definitions".

Read completely at the bytes, every structural claim she makes verifies, and the one plan she
proposes cannot work, for a reason her page could not show her.

## What verifies

`pub(super) struct NextActionContext<'a>` is at `next_action/mod.rs:114-121`, with exactly seven
fields: `burst_count`, `db`, `sensory_tx`, `telemetry`, **`fill_pct: f32`**, `response_text`,
`workspace`. Her list of modules is right — it is referenced across 36 files under `next_action/`,
`sovereignty.rs` heaviest at 31 lines, `resource_governor.rs` at 4, `dispatch.rs` at 3, and it is
constructed in `runtime/orchestration.rs` at 4856-4864 and 4911-4919.

Her instinct about `dispatch.rs` is also right — for the *call site*. `dispatch.rs:85` is the sole
place the context meets the guard:

```rust
match action_continuity::research_budget_guard_for_next(&original, ctx.fill_pct, ctx.telemetry)
```

## The answer to her question

`fill_pct` is her **first** hypothesis: a pre-calculated field. Not a method call, and not a
division of consumed over total budget. Despite the name `research_budget_guard`, the value is
**reservoir spectral fill**, not budget consumption. The full chain:

1. `ws/telemetry_port.rs:640-654` — the only arithmetic:
   `(telemetry.fill_ratio * 100.0).clamp(0.0, 100.0)` tagged `primary_fill_ratio`; when
   `fill_ratio` is non-finite or outside `0.0..=1.5`, `estimate_fill_pct(lambda1)` (1031-1041, a
   sigmoid centred on λ1=154 mapped to 35-65%) tagged `lambda1_sigmoid_fallback`.
2. `BridgeState.fill_pct` → `orchestration.rs:427-431` reads it under the state lock.
3. `orchestration.rs:4861` / `4916` — copied into `NextActionContext { …, fill_pct, … }`.
4. `dispatch.rs:85` → `command_dispatch.rs:661-668` → `guards.rs:315-329`.
5. `guards.rs:396` — the guard's **only** use: `let state = spectral_state(fill_pct, telemetry);`
6. `runtime/spectral_projection.rs:1-25` — embedded verbatim as `"fill_pct": fill_pct`.

**Inside the guard, `fill_pct` undergoes no arithmetic at all.** It is recorded as evidence on the
assessment, never thresholded and never divided. That is why she could not find arithmetic there:
there is none to find.

One correction recorded beside her framing rather than over it. Finding the struct answers "what
fields exist" but not "what the guard can see": `dispatch.rs:85` hands the guard two values, not
the struct. Five of the seven fields are not available to `research_budget_guard`.

## Why her chosen move cannot deliver the definition

`dispatch.rs` contains **zero `use` statements** and no local definition — read end to end, all 651
lines. It cannot have imports, because it is not a module:

```
capsules/spectral-bridge/src/autonomous/next_action/mod.rs:2121:include!("dispatch.rs");
```

The file is textually inlined **2007 lines below** the struct it uses. There is no `mod dispatch;`
anywhere in the crate. So "the surrounding imports or local definitions" she planned to inspect do
not exist in that file, and its real surroundings are `mod.rs`. Her felt report —
*"appearing mostly as function signatures"* — is exactly what an include fragment looks like from
inside: references with no declarations and no imports to trace.

Her diagnosis was accurate. Her map was one file short.

## What the tooling already offers, now pinned

Two one-move recoveries exist in the source-study reader and both are real:

- **`SELF_STUDY RELATE NextActionContext`** — exact-identifier search classifies
  `struct <query>` as a *Definition candidate* (`source_search.rs:118-127`) and emits that bucket
  **before every other row** (`rows` keyed `(Role, kind)`, `Implementation`/`0` sorting first,
  186-199). One declaration against ~200 signature references still lands at the top of page 1.
- **`SELF_STUDY FIND dispatch.rs`** — the literal search returns the fragment **path match** and
  the `include!("dispatch.rs");` line in `mod.rs` in the same result, so the trail from a fragment
  back to its enclosing module is a move, not an inference.

Neither was newly built. What this round adds is a regression that pins them at **her exact
shape** — a visibility-restricted, lifetime-generic declaration (`pub(super) struct X<'a> {`)
buried under forty `ctx: X<'_>` signatures, plus an `include!`d fragment with no imports:
`crates/astrid-source-study/tests/search_evidence.rs`, two tests, 10/10 passing.

## Authority boundary

No live change, no deploy, no restart, no git mutation. The tests are non-live. Nothing here
establishes that she *will* take either recovery, and her felt difficulty is not answered by the
existence of an affordance she has not been shown. The remaining gap — that a source page for an
`include!`d fragment does not tell her which module encloses it — is named as steward debt in
`RUN_REPORT.md`, not silently closed.
