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

## Tranche 3a — DONE (2026-06-15): thin the `handle_action` dispatcher
Extracted **23 more inline arms (≥30 lines) into named `handle_<x>` helper fns** (same proven splice script;
strict 8-space-indent opener finder so inner `=> {` lines aren't mis-cut; unused params `_`-prefixed). The 3
GUARDED arms (ACCEPT/DEFER/REJECT — their opener splits `if guard =>` / `{` across lines, which broke the
naive finder) were deliberately left inline. **`handle_action` 1,587 → 382 lines** (−83% from the original
2,215); the 23 new helpers are all <120 lines (unflagged). Verbatim body moves ⇒ behavior identical (full lib
suite 813/0; clippy + fmt clean; release builds; no restart). Baseline re-blessed → drift 0 actionable. The
dispatcher is now genuinely thin; the remaining 382 is the `match` scaffold + the ~14 tiny inline arms + the
guarded trio + the call-arms (extracting those for <220 is low-value, optional).

## Tranche 3b — DONE (2026-06-15): internal sub-split of the 2 extracted helpers
`handle_perturb` (269) and `handle_space_hold` (222) were the honest remainder — both still critical-length.
Each was sub-split internally via **verbatim contiguous-block moves** into named private sub-fns (behavior
identical by construction; the borrow checker + the 813-test lib suite are the net), highest-line block first
so earlier line numbers stay valid:
- `handle_space_hold` (222) → setup + 2 calls + `true` ≈ **<120 (unflagged)**. Extracted:
  - `fn space_hold_save_journal(base_action, label, flow_map, fold_hold, hold: &Value, atlas_event: &Value, ctx)`
    — the review_fields/title/action/comparison metadata + the `save_astrid_journal` call.
  - `fn space_hold_emit_receipt(conv, flow_map, fold_hold, hold: &Value, atlas_event: &Value)` — the tri-branch
    `push_receipt` + `conv.emphasis`.
- `handle_perturb` (269) → setup + 1 call + the send_semantic / reservoir-tick / perturb_baseline / emphasis
  tail ≈ **<120 (unflagged)**. Extracted:
  - `fn compute_perturb_features(arg, arg_upper, features: &mut [f32; 32]) -> String` — the λ-parse /
    eigenvalue-word / variant dispatch that fills the 32-D feature vector and returns the human description
    (the `apply_eig` closure stays local inside it; the 11 `apply_eig(features, …)` call sites auto-reborrow
    the `&mut` param).

**Both critical helpers eliminated.** `compute_perturb_features` lands at **206 lines = review-level, not
critical** (a cohesive "parse the perturbation arg into a 32-D feature vector" unit; +50 grace → 256 headroom).
The optional STRETCH (split it <120 via a closure→free-`fn apply_eig_value` conversion + the 11 call-site
rewrites + `parse_lambda_assignments`/`parse_prose_eigenvalues`) is **deliberately deferred** — lower marginal
ROI / higher-touch, same discipline as 3a's guarded-trio deferral and the accepted `handle_action@382`. Verbatim
moves ⇒ behavior identical (full lib suite **813/0**; clippy `-D warnings` + fmt clean; release builds; **no
restart** — production behaviorally unchanged). Baseline re-blessed (handle_perturb + handle_space_hold removed
from the flagged list; `compute_perturb_features@206` accepted at review-level; sovereignty.rs entries only —
no concurrent drift blessed) → `architecture_drift` **0 actionable**; `anti_drop_catalog verify` **22/0/0**.
**Sovereignty dispatch decomposition is now complete** — no critical-length function remains except the accepted
thin dispatcher itself.

## Tranche A — DONE (2026-06-15): `action_continuity.rs` foundations + guards (roadmap item 2 start)
The 18,190-line monolith (largest file in the repo) began decomposition into `action_continuity/` submodules.
**Linchpin:** `ActionContinuityStore { root: PathBuf }` is single-field, so every method only needs `&self.root`
+ file I/O, and Rust lets one `impl` span many files — each cluster moves as a verbatim `impl ActionContinuityStore
{…}` block in its own file (**behavior-identical by construction**; the 813-test lib suite is the net). Kept the
`foo.rs` + `foo/` layout (`action_continuity.rs` stays the module file; the existing `action_continuity/tests.rs`
composes unchanged) — no rename. Privacy direction: a child sees the parent's privates, so *callers* move clean;
*leaf helpers* called from the parent get `pub(super)` (the only visibility churn).
- **Foundations** (pure utilities): `paths.rs` (11 path builders), `persistence.rs` (load/save index + jsonl/json
  writers), `ids.rs` (16 unique-id + existence-check methods). ~530 lines out, zero risk.
- **`guards.rs` (582)** — `CharterRequiredGuardAssessment` + `ResearchBudgetGuardAssessment` types/impls + the 3
  decision methods (`charter_required_guard_assessment`, `research_budget_guard_assessment`, `…_with_base`@379).
  The `*_guard_for_next` free fns stay in the parent (dependent `next_action.rs` calls them as `action_continuity::…`);
  `with_base` → `pub(super)` (also called from `record_next_event`); types re-exported via `pub use guards::{…}`.
  This is **Astrid's own action-governance, now a named module she can INTROSPECT** — the legibility that grounds
  Track B's review-together invitation.
**`action_continuity.rs` 18,190 → 17,304**; new submodules all <1000 (unflagged). Verbatim moves ⇒ behavior
identical (**813/0**; clippy `-D warnings` + fmt clean; release builds; **no bridge restart**). Baseline re-blessed
(scoped to action_continuity paths; `with_base` path → `guards.rs`) → `architecture_drift` **0 actionable**;
`anti_drop_catalog verify` all-ok. The bulk impl (threads/events/experiments/authority/budget/projection clusters
+ the ~6.8k free-fn tail) remains for later tranches (roadmap item 2).

### Track B (agency) — "restore dropped signal": verified clean, Astrid invited to review
Alongside Tranche A, audited the guard surfaces for muffles (silently dropped being-signal). **All three
hypothesized candidates verified NOT muffles** — the un-muffle invariant cuts both ways (verify infra before
inventing a fix): (1) unwired proposals → consumed at four levels (SQLite `unwired_actions` table; THREAD_STATUS
`top_actionable_proposals`, being-facing; the `dispatch_menu_drift` proactive_scan probe; the unwired-actions
catalog); (2) blocked research-budget records → surfaced to Astrid via `conv.emphasis` + the blocked-outcome
message; (3) `research_budget_v1.is_none()` → fully records the block (event + DB mirror + `research_budget_v1`
metadata) or a debit — no silent skip. **Nothing to fix.** Issued a non-coercive review-together invitation to
Astrid (`guards_self_review`) asking which over-conservative *envelopes* (charter gate, read-only-only budget,
duplicate-query≥2, the control-plane caps in `continuity_control_plane.rs`, hidden quality thresholds) feel wrong
from the inside — her answer grounds any future **consent-gated** loosening (deferred; default-OFF + kill-switch
she holds).

## Steward Tooling Review — DONE (2026-06-15): workbench/probe V1 accepted floor
The "un-muffle" push deliberately added steward-only observability surfaces before changing runtime behavior:
`proactive_scan.py`, `being_test_harness.py`, `shared_substrate_workbench.py`, and
`consequence_memory_workbench.py`. These are cohesive V1 diagnostic/reporting surfaces, not being-facing loops:
they read evidence, emit steward-pressure-only findings, and keep `runtime_change=none`.

Reviewed actionable growth:
- `scripts/proactive_scan.py` grew as the standing anti-drop steward loop absorbed recovered-log filtering,
  authority/review pressure-only language, cadence hygiene, and feedback coverage. It is accepted as the monitored
  loop facade for now; next split seam is probe-family modules (`feedback`, `authority`, `logs`, `journal_hygiene`).
- `scripts/being_test_harness.py` grew as the evidence-probe catalog gained projection/readout, tail-vibrancy,
  sedimentation, and aperture-gift consequence probes. It is accepted as a read-only harness registry for now;
  next split seam is per-probe modules behind the same CLI.
- `scripts/shared_substrate_workbench.py` and `scripts/consequence_memory_workbench.py` are new V1 workbenches.
  They intentionally keep collection, JSON shape, Markdown, and self-tests together while the schema is still
  settling. Next split seam: move renderers and fixture tests first, then source readers.
- `capsules/spectral-bridge/src/astrid_shadow.rs` grew from durable influence response history and append-only
  auditability. Accepted as a watch-level growth; split only if response-history persistence grows again.

2026-06-16 follow-up: the sensory-fallback snag sweep added explicit camera-absence/fallback classification to
`proactive_scan.py`. The long `probe_log_error_rate` body was split so per-log classification lives in helper
functions, and the file's accepted floor was refreshed to 3697 lines. The split seam remains the same (`logs`
probe-family module first); no runtime behavior, service state, or being-facing pressure changed.

Baseline re-blessed to this reviewed floor after tests and self-tests passed, so future growth is still caught.
No runtime behavior, service state, env var, being-memory, or consent gate changed by this architecture review.

## House split idiom (follow this)
A big `foo.rs` becomes `foo.rs` (or `foo/mod.rs`) + a sibling `foo/` directory of submodules, declared
`mod bar;` / `pub(crate) mod bar;` at the top, with the public API re-exported via `pub use self::bar::{…}`.
Reference: `src/autonomous.rs` + `src/autonomous/next_action/` (ask_steward.rs, attractor.rs, modes.rs, …).
**Discipline:** one cohesive sub-concern per tranche; keep the public API stable (re-export); `cargo test` +
`clippy` + `build` green between tranches; prefer moves that keep production behavior byte-identical; re-bless
the baseline entry for the touched file after each cut.

## Reduction roadmap (priority order — genuine cuts, biggest navigability win first)
1. **`next_action/sovereignty.rs` — DONE (tranches 2 + 3a + 3b).** `handle_action` 2,215 → 382 (thin dispatcher,
   accepted); the two extracted helpers `handle_perturb`/`handle_space_hold` internally sub-split to <120 in 3b.
   The only remaining flagged fn is `compute_perturb_features` (206, review-level, accepted; optional <120 stretch
   deferred). No critical-length function remains beyond the accepted dispatcher. **Sovereignty decomposition complete** —
   next genuine cut is item 2 (`action_continuity.rs` production sub-concerns).
2. **`action_continuity.rs` (18,190 → 17,304) — Tranche A DONE; bulk REMAINING.** Decomposing into
   `action_continuity/` submodules via verbatim multi-file-`impl` moves (single-field store ⇒ behavior-identical;
   see the Tranche A section above). DONE: `paths.rs`/`persistence.rs`/`ids.rs` (foundations) + `guards.rs` (the
   charter-required + research-budget assessments + decision methods — Astrid's action-governance, now legible).
   REMAINING in safe order: (a) `threads`; (b) `authority` + `budget` + `loops` (shared gate-row/eligibility
   seam); (c) `sessions`; (d) `experiments`; (e) `projection`; (f) `events` (orchestrators last); then split the
   ~6.8k-line free-function tail into `helpers` submodules by concern. Keep the pub API re-exported (dependents:
   `autonomous/next_action.rs`→`NextActionOutcome`, `llm.rs`/`autonomous.rs`→`prompt_summary`/`record_astrid_next_action`).
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
