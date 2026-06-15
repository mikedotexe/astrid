# Architecture Decomposition Plan — bounded tranches to chip down the large files

*2026-06-13. The honest record behind the architecture-health baseline refresh. The `architecture_drift`
probe (`scripts/architecture_health.py`, baseline `scripts/baselines/architecture_health.json`) was
chronically reporting ~16 "actionable" signals. This plan is the agreement that the current large files are
**reviewed, accepted known debt** — scheduled for reduction in bounded tranches, not hidden. The baseline
refresh silences FUTURE drift detection from today's floor; this doc is why that's honest.*

## Why a baseline refresh is honest here (not silencing)
The probe is differential: "actionable" = grown past the baseline grace or newly crossing a threshold. The
accumulation was real engineering (the un-muffle program, transparency track, steward-loop guardrails) — not
bloat. Splitting every large file at once would be a risky big-bang. So: **do a real reduction as a
down-payment, accept the rest as reviewed debt, and record the reduction order here.** Each future tranche
makes a real cut, then re-blesses the touched baseline entries, so the probe always catches *new* drift.

## Tranche 1 — DONE (2026-06-13): carve `action_continuity.rs`'s test module out
- `#[cfg(test)] mod tests { … 4,941 lines … }` → `src/action_continuity/tests.rs` (Rust 2024 dir-submodule;
  `mod tests;` in the parent). `action_continuity.rs` **23,132 → 18,190**.
- Production code byte-identical (tests are `#[cfg(test)]`, excluded from release) ⇒ no behavior change, no
  restart. 78 action_continuity tests pass from the new file; fmt + clippy + release green.
- Baseline re-blessed to the current floor (action_continuity@18,190 accepted; tests.rs@4,911 accepted as a
  test file). `architecture_drift` actionable → 0.

## Tranche 2 — DONE (2026-06-15): split `handle_action` (sovereignty.rs), first cut
The 2,215-line `handle_action` (a `match base_action` with ~39 self-contained arms) had its **3 biggest,
hairiest arms extracted into named private helper fns** in the same file (house idiom: dispatch stays a
`match`, the arm calls `handle_<x>(conv, base_action, original, ctx)`):
- `PERTURB | PULSE | BRANCH` (264 lines) → `fn handle_perturb`
- `SPACE_HOLD | …` (218) → `fn handle_space_hold`
- `NATIVE_GESTURE | RESIST` (153) → `fn handle_native_gesture`
**`handle_action` 2,215 → 1,587 lines.** Verbatim body moves ⇒ behavior identical (full lib suite 813/0;
clippy + fmt clean; release builds; no restart — production behaviorally unchanged). Baseline re-blessed.
**Tranche-3 follow-up (genuine debt, not done):** `handle_perturb` (269) and `handle_space_hold` (226) are
themselves still critical-length — they need INTERNAL sub-splitting (handle_perturb's λ-parsing / `apply_eig`
closure / inner `match key_up` are natural seams). And `handle_action` (1,587) is still large: continue
extracting the medium arms (the ~50–77-line cartography/audit/viz/param arms → unflagged <120-line helpers)
toward a thin dispatcher. NOTE: after this cut, line numbers shifted — re-map current arm boundaries before
the next extraction (the same proven python splice script, fed the current ranges).

## House split idiom (follow this)
A big `foo.rs` becomes `foo.rs` (or `foo/mod.rs`) + a sibling `foo/` directory of submodules, declared
`mod bar;` / `pub(crate) mod bar;` at the top, with the public API re-exported via `pub use self::bar::{…}`.
Reference: `src/autonomous.rs` + `src/autonomous/next_action/` (ask_steward.rs, attractor.rs, modes.rs, …).
**Discipline:** one cohesive sub-concern per tranche; keep the public API stable (re-export); `cargo test` +
`clippy` + `build` green between tranches; prefer moves that keep production behavior byte-identical; re-bless
the baseline entry for the touched file after each cut.

## Reduction roadmap (priority order — genuine cuts, biggest navigability win first)
1. **`next_action/sovereignty.rs::handle_action` — was 2,215 lines; now 1,587 (tranche 2, 3 biggest arms
   extracted).** REMAINING (tranche 3): internally sub-split `handle_perturb` (269) + `handle_space_hold`
   (226), and continue extracting the medium arms toward a thin dispatcher. Function-level, lower blast
   radius than a file-wide reshape; verbatim-move discipline keeps behavior identical.
2. **`action_continuity.rs` (18,190) production sub-concerns** → `action_continuity/` submodules, in safe order:
   (a) the Guard Assessments (`CharterRequiredGuardAssessment` + `ResearchBudgetGuardAssessment`, ~lines
   346–577) — small, cohesive, easy first production extraction; (b) the authority/charter/lifecycle helpers
   (~12,300–18,180); (c) the `ActionContinuityStore` impl (the bulk) last. Keep the 14 pub fns + 12 pub structs
   re-exported (dependents: `autonomous/next_action.rs`, `action_self_knowledge.rs`, `llm.rs`).
3. **`autonomous.rs` (8,620)** — the `spawn_autonomous_loop` fn is ~3,605 lines; extract phases into submodules.
4. **`codec.rs` (4,917)** / **`llm.rs` (4,175)** / **`autonomous/next_action.rs` (3,871)** — cohesive feature
   extractions (codec layers; llm provider/fallback; next_action sub-handlers).
5. **`protected_diagnostics.rs` (2,624)** — the payload builders (`latent_stasis_payload`,
   `resistance_gradient_payload`, etc.) into a `payloads` submodule.

## Deliberately-large — accept, do NOT split (registries / schema / tests)
- `types.rs` (2,606) — the canonical IPC/telemetry schema/registry (CLAUDE.md exempts centralized registries).
- `action_continuity/tests.rs` (4,911) and other `*_tests.rs` — test suites; large is expected.
- The steward scripts (`proactive_scan.py`, `self_study_review.py`, `being_test_harness.py`,
  `astrid_model_canary.py`) — split opportunistically if a cohesive seam appears, not on a line-count clock.

## Maintenance
After any real reduction, re-run `python3 scripts/architecture_health.py --json` and re-bless the touched
entries in `scripts/baselines/architecture_health.json` (the `_blessed` note records the last refresh). The
probe then keeps catching genuine NEW drift while this plan tracks the deliberate paydown.
