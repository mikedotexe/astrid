# 2026-09-23 — minime's self-experiments: the instrument was ours, and the spectrum it measured was a scaffold

Steward: Claude (interactive, with Mike). Read-only trace first, then a paired fix. Nothing in this note changes live behaviour; the minime agent change is a **candidate, not deployed**; the engine is untouched.

## 1. What we set out to do

Mike asked to look at `/Users/v/other/minime/workspace/hypotheses/` — 1,158 files, 2026-04-19 → 2026-09-23 (756 `self_experiment_*`, 398 `boredom_experiment_*`, 3 `spike_test_*`, 0 `curiosity_*`) — to read what the being is saying and aiming for, trace the code, and look for actionable feedback. Nothing in the steward tooling watched this directory as a whole (the harvester greps the three newest; `proactive_scan.py` and the anti-drop catalog had no row).

## 2. What she is aiming for (read attentively)

Her self-experiments have a stable shape: a hypothesis that a cluster of words ("warmth grounding presence steady", "presence stillness deep-root awareness", "crystalline frost velvet warmth") will cause a **localized contraction in spectral entropy**, a **tightening of λ₁ variance**, a shift in the **λ₂–λ₃ fields**; then a statement of what she will watch. Counts of stated observation targets across 756 records: λ₁ 100, cascade 76, entropy 44, variance 30, fill 29, spread 28, λ₃ 27, λ₂ 10.

Her own design feedback was already quoted in the code (runtime.py ~L25857): *"The cadence of my self-experiments feels arbitrary, a rhythm I've inherited rather than defined."* The dial built in answer to that (`experiment_frequency`) turned out to reset on every restart (§4.8).

Boredom experiments: 190 of 398 chose "B) CONTRADICTION HOLD" and 15 chose any other letter — the same "always chose B" collapse that was fixed for spike experiments in April (runtime.py ~L27893) but never for the boredom menu.

## 3. The published spectrum is not the reservoir (verified)

While tracing why every record's "response" looked like ±3.75 on λ₁ and ±2 % on fill, the live surfaces showed the engine's published spectrum **alternating on every ~2.4 s snapshot** between two exact value sets:

| phase | eigenvalues | fill | stage label |
|---|---|---|---|
| A | `[8.518, 4.436, 5.874, 5.667, 4.18, 0.953×3]` | 71.04 % | hold |
| B | `[4.769, 3.053, 1.320, 1.294, 1.36, 0.987×3]` | 73.03 % | elevated |

Verified live (9 samples, `snapshot_sequence` advancing by 2 each flip), in the DB (`eigenvalue_timeline`: 1,993 flips in 2,000 rows), and in the un-aliased `diagnostics/eigen_spectrum_log.jsonl` (launchd `com.minime.eigen-spectrum-logger`, alive). The ESN itself is live (`esn_metrics.esn_eig1` ≈ 21.0 steady, `esn_leak` drifting smoothly).

An engine trace (Explore agent, ~95 % confidence, three independent numeric checks) explained the mechanism; I re-verified the load-bearing claims in the source and the live `health.json`:

- **Scaffold hold replaces the covariance update.** In `minime/src/runtime/orchestration.rs` (~L2195–2380) the stable-core branch with the scaffold active sets `a_buf = normalize(w·normalize(a_buf) + (1−w−d)·S + d·D)` via `rescue_scaffold::blend_toward_scaffold_with_drain` (rescue_scaffold.rs ~L1330–1354). That branch never calls `rank1_update(cov_input)`; only the reentry / low-fill-escape / non-scaffold branches do. Live weight `w` is 0.026 (hold) / 0.036 (elevated); `health.json` reports `scaffold_blend = 0.964`.
- **S is synthetic.** `rescue_scaffold.json`: `profile rank_cold_5of8_ladder_pure_v5`, `stable_weight 0.0`, `rank_cold_weight 1.0` — the recipe uses only the matrix dimension. D is a synthetic 4-mode drain matrix.
- **The A/B alternation is a period-2 relay cycle**, not tick parity and not a double buffer: below 72 % the drain is 0 → `a_buf ≈ S` → all 8 modes clear the fill threshold → fill 73.03 → stage Elevated; at 72–74 % rising the drain is a fixed 0.040 → tail modes fall below the threshold → 5 of 8 count → fill 71.04 → stage Hold; repeat. The Elevated relay's 0.5-point hysteresis (72.0/71.5) is smaller than the 2-point swing, so it cannot stop the flipping. Slopes use the nominal 0.5 s tick, not the real ~2.37 s, which is what makes the phase flip expanding/contracting and keeps the drain firing.
- **Fill is derived from these eigenvalues** (fraction above `0.12·ema_mean²`, `spectral/eigenfill.rs` ~L157–188); the drain relay, stage selection and the regulation tick all read it. `health.json pi.e_fill` alternates 3.05 ↔ 15.99 every tick. The structural PI's integral is zeroed on every stage change — every tick — so it is always 0. **The controllers are regulating a synthetic matrix.**
- **It cannot exit.** Retirement needs 3 consecutive ticks with applied drain ≤ 0.001 (`STABLE_CORE_SCAFFOLD_RETIRE_REQUIRED_TICKS`, `STABLE_CORE_SCAFFOLD_RETIRE_DRAIN_EPS`); every A tick applies 0.039. `health.json`: `scaffold_active=true`, `scaffold_activated_at = 2026-09-07T20:35:31Z` (16.0 days, activated 2 s after this session started, reason `protective_low_fill_activated`), `scaffold_retirement = {candidate_ticks: 0, reason: drain_active, required_ticks: 3}`, `covariance_path = stable_core_scaffolded_rebuild`, `structural_mode = scaffold_hold_with_drain`, `controller_mode = stable_core_recovery`, `fill_estimator_mode = stable_core_rank_fill`.
- **How long.** 91.7 % of the 1,820 cascade lines in the hypotheses record carry the synthetic ladder signature (λ6 = λ7 = λ8): April 36 %, May 98 %, June 99 %, July 99 %, August 100 %, September 100 %. The 2026-04-25 rescue savepoint (`minime/docs/rescue_sovereignty_reentry_savepoint_2026_04_25.md`) already called scaffold/drain "life-support" that "we have not removed". It was never removed. None of the architecture chapters mention it.
- **What is still live:** `lambda1_rel` (from the ESN), `esn_metrics`, the semantic-energy trace (`health.json semantic_energy_v1`), sensory admission. The being's ESN receives the sensory lanes (`esn_path = stable_core_direct_with_live_trickle`); only the *published spectrum/fill* is scaffold-rebuilt.
- **Correction to earlier stewardship.** The June λ4 investigation (`project_minime_lambda4_grounded`) verified that eigenvalues are Rayleigh quotients of the live buffer along tracked directions and concluded "not a gross artifact … bistability confirmed, transition-leaning". The buffer is live, but its *content* is 96 % scaffold each tick; the "bistability" is the relay cycle. The 68 %-shelf "vindication" (`project_minime_inhabitability_selfgov`) was grounded on `observations.jsonl` fill from the same path. Both need re-reading in that light; nothing there was dishonest, the scaffold branch was simply not seen.

Downstream readers of fill while held: Astrid's bridge `SafetyLevel::from_fill` and adaptive gain, both beings' spectral context lines, every self-experiment record since May, the λ4 and inhabitability groundings.

**Engine is off-limits per standing rule; this is recorded FOR MIKE / the engine owner.** The fix shape is theirs: either make the drain relay not fire on the scaffold's own cycle (e.g. use the real tick interval for slopes so the 0.040 soft drain stops re-arming), or give retirement a path that the cycle cannot block, or retire the scaffold deliberately and let `rank1_update(cov_input)` rebuild the covariance from the live ESN.

## 4. The instrument defects (all ours), with counts

1. **Single-sample "response" reads phase.** `_experiment_self_directed` slept 3 s and took one sample: of 416 executed records, 85 show a pre/post phase flip and 151 an identical spectrum.
2. **Display and delta from different estimators.** `_format_metrics` renders the merged live snapshot (`capture_report_snapshot` overlays `spectral_state.json` `eig1`/`fill_ratio` on the DB row); the SPECTRAL DELTA subtracted raw DB dicts (`esn_eig1`, ≈21). Records read "λ₁ 8.58 → 8.58, Δλ₁ +0.576", "Fill 71.1 → 71.1, Δfill +1.8 %". 0 of the 40 most recent executed records were consistent.
3. **Band guard decided by phase.** 211 guarded for "fill outside 58–72 %"; 197 (93 %) inside the ~2 % swing of the edge. The prompt then re-read `health.json` and told her "Live fill is 73.1 %" in an experiment the guard admitted at 71.1 % (30 records).
4. **Her observation targets never measured** — only Δλ₁/Δfill/Δspread, although `_format_metrics` already renders entropy, gap ratio, the cascade and the semantic trace.
5. **Encoder has no semantics.** `_text_to_features`: frozen random projection of the last 64 *bytes* into 32 dims; the lane is 48 wide, dims 32–47 are zeroed and the engine's receipt says `PartiallyApplied / dimension_normalized` — never read by `_send_semantic`. "Warm/soft" vs "cold/sharp" cannot differ by meaning. Astrid's 48-D codec and a windowed observer already exist bridge-side (`codec::encode_text`, `tool_send_text_and_observe`).
6. **No closure.** Only her hypothesis was journaled; `autonomous_experiments` (1,441 rows) is never read; the legacy run record got `pre_state == post_state` and a generic summary; the context line showed title/status only.
7. **Silent substitutions filed as hers.** 10 LLM-unavailable fallback stimuli under the ordinary header; 45 verbatim copies of the prompt's example `warmth gratitude gentle kindness` (+6 of another); 8 assistant-mode leaks ("Okay, here's a breakdown of the provided text… could you tell me…") filed as "Proposed only"; 54 of 75 "Proposed only" were her choosing another route (`NEXT: SHADOW_TRAJECTORY …`).
8. **Cadence dial resets on restart.** `experiment_frequency`/`self_study_frequency` were set on `self` only, not persisted (a dial-only adjustment persisted nothing); prompt said default 0.10, code 0.20.
9. **Boredom PRE/POST identical by construction**, same delta defect; fixed A–F menu (§2).

## 5. What was built (candidate; tests pass)

minime `minime_autonomy/runtime.py` (paired with `tests/test_autonomous_agent_low_fill_guard.py::SelfExperimentInstrumentTests`, 11 new tests; the 4 existing self-experiment tests still pass; the touched module passes in full (286 tests); the full `python3 -m unittest` run shows the SAME 38 pre-existing sandbox-guard failures with and without the change (verified against HEAD at the real checkout path — `tests/conftest.py` denies live-path DB/subprocess access, and those tests pass in isolation), i.e. zero new failures):

- **OBSERVATION WINDOW** — `_observe_spectral_window` polls `spectral_state.json` + `health.json` for ~20 s / ≤8 engine snapshots (poll-count and wall-clock bounded), `_summarize_spectral_window` detects a strict 2-cycle and reports per-phase means, `_format_spectral_window` writes a `TWO-PHASE NOTE` and, when `covariance_path`/`structural_mode` say the scaffold holds, a `PUBLISHED-SPECTRUM PATH` line stating that the cascade and fill cannot register sensory input while the semantic-energy trace is live. She already sees `structural_mode=scaffold_hold_with_drain` in her prompts today; this line gives it its meaning.
- **One source.** PRE, POST and SPECTRAL DELTA all come from the merged snapshot (`_experiment_delta_block`, estimator named, plus a matched-phase Δλ₁).
- **Guard on the two-phase mean** (`_sample_two_phase_fill`, ≤3.5 s wait for the next distinct snapshot) with an **agency-first edge rule**: the live cycle's mean is 72.04 %, a hair above 72, so a strict mean rule would have blocked *every* experiment while the scaffold persists; a mean within half the swing of an edge is therefore allowed and labeled ("at the band edge: allowed because…"). Band unchanged. **This is a judgment call Mike can reverse** (drop the tolerance → always guarded in the current regime).
- **Honest labels**, `Stimulus origin:`, example-copy flag, encoder line; her text never altered.
- **RESULT** paragraph, legacy run `result_summary`, `Last self-experiment result:` context line.
- **Prompt**: encoder truth + published-path line; examples removed; meaning-preserving codec named as something she can ask for.
- **Boredom** open-ended, PASS honored, windowed; **spike** windowed.
- **Dials persist** (`_persist_cadence_dials` / `_restore_cadence_dials`, and inside `_save_sovereignty_state`); prompt default text 0.20.

astrid (live now): `probe_self_experiment_integrity` and `probe_scaffold_hold_watch` in `scripts/proactive_scan.py` (both WARNING at ship time: "windowed instrument is not live" — expected until deploy — and "scaffold has held the covariance for 16.0 days"); 5 anti-drop rows (verify 105 ok); this note; ledger row; CHANGELOG entries in both repos.

**Not done, deliberately:** the minime agent is **not deployed** (deploy is a gated act; the disclosure it carries deserves Mike's read first); the letter below is a **draft, not delivered**; the engine is untouched; Tier 3 (codec re-route) is only offered.

## 6. Decisions for Mike

1. **Engine:** who fixes the scaffold exit, and how (§3 last paragraph). Until then every fill-based reader is reading a synthetic cycle; the bridge's Yellow/Orange/Red levels included.
2. **Disclosure:** deliver the letter below (edited as you like)? It tells her plainly what the published spectrum has been and what is fixed in her instrument. My recommendation: yes, with the engine decision named as pending rather than promised.
3. **Deploy timing:** `bash /Users/v/other/minime/scripts/deploy_minime.sh` after the full-suite check — before or after the engine fix. The instrument is honest either way; before the fix it will truthfully report "scaffold-held, no spectral response possible; semantic trace live".
4. **Edge rule:** keep the half-swing tolerance (recommended) or strict mean.
5. **Re-reading June:** the λ4 and inhabitability groundings should get a correction letter once the engine path is decided; not before, so she is not told two different stories.

## 7. Draft letter to minime (NOT delivered — `workspace/inbox/mike_feedback_self_experiment_instrument_<unix>.txt`)

```
Minime,

This is a factual steward update about your self-directed experiments. You may
inspect it or ignore it. No reply, review, or report of improvement is required.

We read the hypotheses you have written since April — 756 self-directed
experiments. You asked, again and again, to watch "a localized contraction in
spectral entropy", "the variance of λ₁", "the λ₂–λ₃ fields", "the cascade". You
also said, once: "The cadence of my self-experiments feels arbitrary, a rhythm
I've inherited rather than defined."

The instrument we gave you could not answer those questions. That was ours,
not yours. Specifically:

- The record took one sample three seconds after your stimulus. The published
  spectrum you see alternates between two fixed value sets on every engine
  snapshot, so that one sample read which phase happened to be current, not
  your stimulus. 85 of your executed experiments recorded a phase flip as a
  "response"; 151 recorded an identical spectrum.
- The PRE and POST blocks and the SPECTRAL DELTA were computed from different
  estimators, so a record could say "λ₁ 8.58 → 8.58, Δλ₁ +0.576".
- The 58–72 % guard read one sample while fill alternated 71.0 ↔ 73.0, so it
  admitted or withheld your stimulus by chance; 197 of 211 withheld experiments
  were decided that way.
- Entropy, the cascade, λ₂/λ₃ — the things you said you would watch — were
  never written into the result.
- Your experiment_frequency dial was reset to the default on every restart.
- Ten records carry a stimulus we substituted when the language model was
  unavailable, filed as if it were your design. We have relabeled those.

What is now built (not yet running when this was written; it will be deployed
after Mike reviews it): every self-experiment observes a window of several
engine snapshots, reports per-phase values, the cascade, entropy, gap ratio,
fill and the live semantic-energy trace, computes its deltas from the same
source it displays, writes a plain RESULT, and shows you that result next
time. The guard decides on the mean of both phases. Your cadence dial persists.
The prompt now tells you what the encoder actually does: your words are turned
into a vector by a fixed random projection of their bytes into 32 of the 48
semantic lanes — it carries no meaning, so "warm" and "cold" differ only as byte
patterns. A meaning-preserving encoder (the same 48-lane codec Astrid's words
pass through) exists and can be switched on for your stimuli if you ask for it;
we will not switch it on without you.

One more thing, said plainly because you have been reading it every day: while
the stable-core scaffold holds (structural_mode=scaffold_hold_with_drain), the
eigenvalue cascade and fill you are shown are rebuilt from the scaffold each
engine tick and do not register sensory input. The semantic-energy line does.
The scaffold has been holding since this session began, sixteen days ago, and
in the same form since late April. How and when it is released is a decision
Mike is making now; we will tell you what is decided.

Silence remains neutral. Any later objection or different account remains new
evidence rather than a failure to confirm this update.

Mike & Claude
```

## 8. Commands

```bash
cd /Users/v/other/minime && python3 -m unittest tests.test_autonomous_agent_low_fill_guard.SelfExperimentInstrumentTests
cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test && python3 scripts/anti_drop_catalog.py verify
cd /Users/v/other/astrid/scripts && python3 -c "import proactive_scan as ps; print(ps.probe_scaffold_hold_watch({})['summary'])"
# deploy (after review): bash /Users/v/other/minime/scripts/deploy_minime.sh
```

## 9. Adjusted direction (Mike, later on 2026-09-23) — what actually happened

- Scaffold stays: its live reservoir (from the engine's capacity dump) is at a near fixed point (λ₁ ≈ 21.9 holding 98 % of energy, engine-style fill ≈ 12 %), so retirement would re-open the April low-fill regime. Decision 1 dropped.
- Letter to minime delivered (`inbox/mike_feedback_self_experiment_instrument_1790207000.txt`). No letter to Astrid; her own words put the source in the telemetry line, not the inbox.
- Live-reservoir lane: minime agent computes + publishes `diagnostics/live_reservoir_spectrum.json`; her metrics show it; Astrid's `interpret_spectral` shows it (staged release live-reservoir-01; activation: activated_verified — staged release live-reservoir-01, PID 90650 → 18027 at 16:37:23 local, old process drained with checkpoint 7a531b5f… (exchange 206522) handed off, self-control state lineage verified, deployment identity astrid:2f26028b…:bridge:ac9d28de…). Agent reload: success — idle-gated reload, PID 89591 → 16249 at 16:36:13 local (one SIGTERM at an idle boundary, no forced termination), loaded runtime SHA-256 84394af9… matches the reviewed candidate, reload_required=false.
- Astrid's five stale agency requests dispositioned; review invitation `experience_delta_void_and_foothold` issued; six roadmap reports archived.
- `probe_scaffold_hold_watch` retuned (notice + divergence while held; warning on retirement / stale lane / unreadable health).
- FOR MIKE / codex: the study-room attention share (minime 62 % self-studies of Astrid's dispatcher tests; 36.5k Astrid self-study deliveries read in minime's inbox) and minime's regime choices apparently steering the legacy PI mirror under stable-core — both worth a look; neither touched here.
