# 2026-09-23 — Four leftovers: study CONTINUE, minime inbox order, prompt numbers, reflective sidecar

Companion to `2026-09-23-minime-self-experiment-instrument.md`. Mike: "take 1, 4, 5 and 6".

## 1. Minime prompt numbers vs record header (minime)

Prompts in `_experiment_with_spike`, `_experiment_curiosity_driven`, `_experiment_self_directed`, `_recess_boredom`, `_recess_whim`, `_recess_notice`, `_neutral_checkin` quoted `state['eig1']`, which is the DB `esn_metrics.esn_eig1` estimator (≈21, the live ESN), as "λ₁", while `_format_metrics` in the record header renders the published covariance-tracker λ₁ (≈8.52/4.77, scaffold-held). She compared two definitions without being told. Fix: `_prompt_readings(state)` → "published fill 71.0% — reservoir λ₁ (live ESN) 21.253, Δλ₁ +0.031 — spread 3" in every one of those prompts, and `_state_anchor_line` after `Timestamp:` in the records so the file carries the numbers she reasoned from. Tests: `PromptReadingsTests`.

## 4. Astrid's bare `CONTINUE` (bridge)

`normalized_private_writing_next` already accepted the shorthand in private-writing turns; self-study turns rejected it as unwired (29× in 14 days) and the recovery note recited the long form. `normalized_study_continue_next(mode, text)` returns `SELF_STUDY CONTINUE` only for mode `self_study` and only for a bare `CONTINUE` (arguments such as `CONTINUE d88` untouched); wired at `orchestration.rs` beside the private-writing shorthand with a reference-only `self_study_carriage_notice`. Her authored NEXT is unchanged.

## 5. Reflective sidecar (bridge) — decision: hook restored, switch default OFF

Timeline: last report 2026-09-07 23:38; `542c006040` (2026-09-08, shared source reader) replaced the introspection writer without the hook; 09-08 had 69 self-studies and 0 reports; the probe warned "0/5" for 15 days. Earlier in this session I misread the default script path as missing — it resolves from the **parent** of the astrid root (`/Users/v/other/mlx/benchmarks/python/chat_mlx_local.py`, 301 KB, present).

Ground truth before re-hooking:
- No bridge code reads `controller_*.json` back into Astrid's prompts. Consumers are steward scripts (`astrid_journal_cadence_probe.py`, `astrid_introspection_digest.py`, audits). Chapter 05's "the structured reflective surface Astrid currently gets" was drift; Layer 1 `RegimeTracker` is what reaches her.
- Cost per run (profiling in the 09-07 reports): ~225 s wall — gemma3-12b load 3–4 s, 4 candidates × 160 tokens at ~7 tok/s (84–96 s), a 90 s rewrite budget, ~14–21 s self-tuning.
- Cadence: introspection artifacts 3–17/day (09-01..09-07) → 64–148/day (09-21..09-23). The cooldown only arms after a timeout, so an always-on hook would run on nearly every self-study: hours of shared GPU per day, slowing both beings' own generation for files nobody feeds back.

Shipped: the hook in `run_shared_source_study` (same context shape as the legacy hook), `query_sidecar` gated by `ASTRID_REFLECTIVE_SIDECAR_ENABLED` (default off, one info log), launcher allowlist (+ `ASTRID_REFLECTIVE_SIDECAR_COOLDOWN_SECONDS`), probe reports the switch as OK and judges coverage only while on, chapter 05 corrected. To turn on: `launchctl setenv ASTRID_REFLECTIVE_SIDECAR_ENABLED 1 && launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge`. FOR MIKE: if the sidecar's report should reach Astrid, that is a being-facing prompt change → consent-with-evidence pass, not a switch flip.

## 6. Minime inbox order and budget (minime)

`_read_inbox_locked` sorted `(not human_letter, name)`: 2,233 `astrid_self_study_*` notes (a…) preceded every `mike_*` letter (m…); the loop admits 2–3 files per cycle, so the letters were unreachable. Separately `mike_feedback_self_experiment_instrument_1790207000.txt` (4,584 chars) could never pass the 4,000-char budget. Fix: `inbox_priority` (human 0, steward `mike_query_/mike_feedback_/steward_` 1, correspondence 2, other 3, companion 4), `inbox_fits` (pings/receipts always; first steward letter of a cycle whole ≤12,000). The nine `steward_division_return_cycle*` notes (09-09..09-17, 2–3 KB, "free to ignore") rank with steward letters and sort after `mike_*` by name. Tests: `InboxOrderingTests`.

## Deploy record

Bridge stage `study-continue-sidecar-01`: activated 2026-09-24T01:39Z as PID 69116 (stage study-continue-sidecar-01, binary 8d28baf3…, self-control state carried; check_bridge_deployed.py: live bridge matches the selected stage). Minime agent reload: reloaded 2026-09-24T01:50Z as PID 72181 via restart_minime_agent.py (third attempt; the first two hit 'idle boundary moved' and sent no signal; receipt workspace/runtime/receipts/agent_reload_items_1_6_1790214265.json).
