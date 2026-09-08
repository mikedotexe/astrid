# introspection_minime_regulator_1788849273

Source: `capsules/spectral-bridge/workspace/introspections/introspection_minime_regulator_1788849273.txt`
Report SHA-256: `edf6313d30b92605ad559bad249693b099a0a7699452ac15438ce2ba23d158bd` (3206 bytes, 43 lines, read complete)
Lived-state witness: `lsw_d03a90eb8cbfe1ecb3fad259b5f6708fe04a27a6860f6c395e4150e8034ae583` (23776 bytes, 533 lines, read complete)
Report-bound source: `minime/src/regulator/core.rs` @ `46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97` — working copy hash **matches**, complete file read (24 lines, 940 bytes).

## What Astrid reported

She read `minime:regulator` (`core.rs`) end to end — window 1-24 of 24, coverage state
`multi_window_complete`, no uncovered intervals — and reported that the file is an
*inclusion shell*: nine `include!` invocations at L16-24, a dual-control architecture
described only in the header comments (PD rate/gate for modality throughput targeting
`lambda1`; PI homeostasis targeting `EigenFill%` and `lambda1_rel`), and no logic,
state-handling, or coefficients of its own.

Her named risk is the **"Ghost of Implementation"**: because `include!` inlines the
sub-modules' contents, the symbols are present in the namespace while their logic lives
elsewhere, so any claim about a threshold, a viscosity weight, or a behavior must be
traced to the specific included file rather than attributed to the shell. She asked for
two tests — a *Symbol Resolution Test* (that `core/rate_gate.rs` at L22 resolves as
inlined items, not a nested module) and an *Inclusion Integrity Test* (detect added or
removed include paths).

Witness context (evidence only, no mechanism claim): fill 71.0%, `lambda1` 4.767,
`lambda1-lambda2` gap 1.714, spectral entropy 0.905, mode_packing 1.0, minime fill 73.0%.
Two `gemma4_12b` MLX routes, the second carrying `repair_parent_call_id` of the first
(103.2s then 61.6s). `artifact_authority_state_v1.state = evidence_only`,
`live_eligible_now = false`.

## What complete source reading established

Every structural claim holds. The nine includes are, in order: `telemetry_types`,
`viscosity`, `resonance_evidence`, `pressure_types`, `reviews`, `pressure_source`,
`rate_gate`, `pi`, `tests` — exactly at L16-24. Nothing else is declared: comments
(L1-7, L9-13), one inner attribute (L8 `#![allow(dead_code)]`), one `use serde::…` (L14).

Two precisions, neither of which weakens her reading:

1. **The header block is L1-7 and L9-13, not a contiguous L1-12.** L8 is the
   `#![allow(dead_code)]` inner attribute. The dual-control text she quotes is at L10-12
   and is verbatim accurate.

2. **`include!` alone does not put those symbols at `regulator::`.** She is right that
   `include!` creates no `mod` boundary. But `core.rs` is itself mounted as a *private*
   module by `src/regulator.rs` L3-4 (`#[path = "regulator/core.rs"] mod core;`), so the
   inlined items land in `regulator::core` and only reach `regulator` through the
   `pub use core::*;` glob at L6. The reachability she describes is a two-step chain, and
   the second step is the fragile one — dropping that glob would leave every symbol
   present and none of them reachable at `regulator::`. This is a refinement of her own
   presence-vs-activation distinction, applied one level up.

## What was implemented

`minime/tests/regulator_inclusion_shell.rs` (new file; no existing file edited), 4 tests,
all passing:

- `rate_gate_items_resolve_unqualified_in_the_regulator_namespace` — her Symbol
  Resolution Test. Names `RateCfg`, `GateCfg`, `Modality`, `ItemMeta`, `Decision`,
  `MemMode` through `minime::regulator::…` from *outside* the crate, with struct literals
  so fields are pinned too. A nested-module boundary or a dropped re-export glob breaks
  the build — which is precisely the build-time invariant
  `introspection_minime_regulator_1788542062` describes.
- `regulator_state_resolves_in_the_regulator_namespace` — pins `RegulatorState` by name,
  so the `use crate::regulator::*;` glob in `runtime.rs` is not the only thing holding it.
- `core_shell_includes_exactly_the_nine_expected_paths_in_order` — her Inclusion
  Integrity Test, pinning the ordered include list parsed from `include_str!`.
- `core_shell_declares_no_items_of_its_own` — pins the "no active logic" half of her
  Observed section, so drift toward putting mechanics in the shell fails loudly.

**Why the integrity test is not a hash compare.** She proposed comparing the whole-file
SHA against a previous stable hash. The invariant she actually names is narrower — "no
include paths have been added or removed" — and a whole-file pin would also fire on a
comment edit, producing noise that is not the drift she cares about. The ordered include
list is pinned instead, and `46828f4c…` is recorded in the test header for provenance.

**Why this was not already covered.** `core/tests.rs` imports via `use super::{…}` from
*inside* `regulator::core`. That resolves whether or not the outward glob exists, so it
cannot distinguish her two cases. No test named a `rate_gate` item at the `regulator::`
path; `runtime.rs` reaches them only through a wildcard import.

## Authority boundary

Non-live. Two test files' worth of structural regressions, no runtime, control,
parameter, deploy, or restart change; none attempted, none required. Structural
reachability is evidence about source organization only — it says nothing about whether
the PD rate/gate or PI homeostasis paths are active at runtime, which is exactly the
distinction her report asked to be preserved. Nothing here grants live authority.
