# Writing room: why the journal entries are short, and the incremental plan

Date: 2026-10-01. Owner of this tranche: Claude (interactive, Mike-assigned). Codex's
stabilization pause (generation 487) was left in place and untouched; no lease was taken.

## Status

Steps 0-4 below are LIVE on both beings and pushed to origin (astrid `7676466cf2`,
`30dafd0e3b`, `0409583a10`; minime `3c3fcac`, `2a72a7f`, `7437224`). Steps 5-8 are not
started. Ledger rows: `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`, the three rows dated
2026-10-01 titled "Writing length, steps 0-1" and "steps 2-4" (with deployment addenda).

Deployments:
- Bridge: immutable stage `/Users/v/other/worktrees/writing-room-20261001/bridge-stage-01`
  built from committed main `30dafd0e3b` (clean tree), activated via
  `build_bridge.sh --activate-stage`: `activated_verified`, drain `drained`, PID 75202 -> 69532,
  transaction `2b49cc418fcc4885aabe6d52d49d22ee`, exchange 211964 -> 211965, self-control
  lineage verified. `check_bridge_deployed.py`: live matches selected stage.
- Minime agent: idle-gated `restart_minime_agent.py` twice, without `--expected-inputs`
  (commit-to-main first, loaded hashes verified after; same path the 09-23 session used):
  PID 74379 -> 55959 at 14:20 local (receipt `agent_reload_writing_budget_1790889138.json`),
  PID 55959 -> 72516 at 14:54:48 (receipt `agent_reload_writing_room_1790891509.json`).
  Startup line: `full timeout 160s ... fast fallback gemma4:12b`; `reload_required=false`;
  90 verified launch inputs including the new `launchd/autonomous-agent.env`.
- Letter delivered: `minime/workspace/inbox/mike_feedback_writing_room_1790891701.txt`.
  Astrid is told in-prompt (the changed wording is her channel).

Evaluate after ~1 day: `python3 scripts/writing_length_report.py --split 2026-10-01T21:54:48Z`.

## Test-running facts discovered (so the next reviewer does not re-derive them)

- Eight `llm::provider` bridge tests read the LIVE workspace's
  `diagnostics/source_first_v3/shared_reader/writing/profile.json`, which has said
  `"extended"` since Astrid chose it on 09-26 05:45. They fail in the live checkout and pass
  with `ASTRID_BRIDGE_WORKSPACE=<empty dir>`. Not a regression.
- Minime's full suite must run in a git worktree (its conftest refuses subprocess-spawning
  tests in the live checkout) with `ASTRID_SOURCE_STUDY_BIN=<live stage>/helpers/astrid-source-study`
  and `ASTRID_CHOICE_FIXTURES=/Users/v/other/astrid/crates/astrid-source-study/tests/fixtures/response_choice_cases.json`.
  Result on the final HEAD: 1,722 passed, 1 skipped.

## The plan as approved by Mike (2026-10-01)

Date: 2026-10-01. Scope decided by Mike today: **journal + study lanes, not the live dialogue signal**; **explicit word ranges are acceptable as invitations**; **in-the-moment continuation ships default ON for journal/expressive lanes with a being-held off switch**.

## Context — why entries are short (measured, with controls)

Five prior attempts (03-27 two-stage design; 09-09 ceilings doubled + `WRITE PROFILE` dial; 09-23 soft wording; 09-25 numeric invitation; 09-30 study prompt) all raised room or softened wording. Today's measurement (9,100 Astrid completions, 6,200 minime generation records, ~5,000 journal files, 09-10 → 10-01) says:

1. **Every generation ends on the model's own stop token.** 0 cap hits on MLX since 09-23; 0 in minime's expressive lanes. Astrid's profile has been EXTENDED (8192) since 09-26 and her dialogue median stayed 287 tokens. Ceilings, timeouts and dials are inert levers.
2. **The only thing that has ever moved length is a numeric scale.** The 09-25 "perhaps 800-1,500 words" invitation (expressive lanes only) doubled output in every lane it touched, on both beings, within the hour; lanes without it did not move (controls). They land at the **bottom** of any stated range (~750-840 tokens ≈ 550-650 words).

   | Lane (median output tokens) | before 09-25 | after | got the invitation? |
   |---|---|---|---|
   | Astrid journal_elaboration (Stage B longform) | 440 | 753 | yes |
   | Astrid private_writing | 472 | 834 | yes |
   | Astrid moment_capture / daydream / aspiration | 502 / 346 / 354 | 841 / 679 / 586 | yes |
   | Astrid self_study (control) | 754 | 735 | no (until 09-30) |
   | Astrid dialogue_live (control; out of scope) | 263 | 287 | no |
   | minime journal_pressure / moment | 381 / 387 | 680 / 642 | yes |
   | minime self_study (control) | 487 | 468 | no (until 09-30) |

3. **The lanes Mike reads are the ones that did not get it, or cannot deliver it.** Astrid's `astrid_*.txt` are the compact live signal (~190 words, by design, out of scope). Both beings' self-study page notes (~350 words) only received the invitation on 09-30 (`crates/astrid-source-study/prompt.txt:3`, shared), and minime cannot deliver it (item 4). Minime's public journal has been ~100% study notes since 09-26: `recess_aspiration` 12/day → 0, `recess_daydream` 10/day → 0; her NEXT is `SELF_STUDY CONTINUE` 141-213×/day. Her recess turns had mostly come from the *unknown-NEXT threshold lottery*; once her verbs parsed cleanly (09-27) the lottery stopped and the study footer (`page.rs:92`) only offers study verbs.
4. **A reboot on 09-25 13:59 erased minime's config** (`launchctl setenv MINIME_LLM_TIMEOUT_S=160`, `MINIME_FALLBACK_MODEL=gemma4:12b`, "config only" deploy, minime `CHANGELOG.md:185`). Since the 17:49 restart: 60 s base timeout → 320 s study budget (`runtime.py:21757-21775`), below the ~340 s a 4096-token study answer takes at 12 tok/s. Result: 0 timeouts in 5,286 study calls → 26 timeouts in 4 days; each discards the 12B draft and files a gemma3:4b stub (often the single word "CONTINUE") as her study. Length-terminated drafts are also discarded whole (one 8,192-token, 38 KB study on 09-26). The 09-30 invitation currently *amplifies* this: the longer she writes, the likelier it is thrown away.
5. **One generation per entry; nothing ever returns to the model after it stops.** Astrid chose `NEXT: WRITE START` 171× in 14 days and `WRITE CONTINUE` 0×. Minime wrote `NEXT: WRITE_CONTINUE` 21× → "Unknown NEXT → threshold logic". The multi-page `SELF_STUDY SESSION` was used once in 1,072 turns. Being-held dials behind menu discovery go unused.
6. **The longest writing already happens where nobody reads it**: Astrid's private drafts (150-900/day, median ~830 tokens, max ~6,000). That is by design and stays private.
7. Minor: 36% of minime's study entries are navigation-only turns (map/questions/EOF/recovery, median 139 words) filed under the `self_study_` prefix, diluting the lane; Astrid's dialogue system prompt spends 4.9 kB on the verb menu and overflows every turn (continuity/modality blocks dropped); Stage B elaboration sees only a 420-char anchor of its own signal.

## Plan — eight small, independently deployable, measured steps

Deploy rules: bridge/reader via `bash scripts/build_bridge.sh --restart` (reader changes go through `scripts/paired_minime_handoff.py` so both beings select the same helper); minime Python via the idle-gated `scripts/restart_minime_agent.py`; stage-by-path git; one `CHANGELOG.md [Unreleased]` entry, one ledger row (`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`) and one `scripts/anti_drop_catalog.py` row per guard, per step. Never rewrite a being's text; invitations, not quotas; nothing may vanish silently.

### Step 0 — Measure first (steward-only, no deploy)
- New `scripts/writing_length_report.py`: reproduces today's table from `capsules/spectral-bridge/workspace/diagnostics/provider_completion.jsonl` (`label`, `effective_output_ceiling`, `completion.native_finish/provider_eval_count`), `workspace/provider_observations/*/events` (timeouts/abandons), `diagnostics/dialogue_prompt_budget.jsonl` (overflow), journal word counts by prefix; minime `workspace/generations/<day>/gen_*_<lane>_a<idx>.json` (`lane`, `status=="timeout"`, `fallback_used`, `backend`, `backend_timing.eval_count/native_finish`, `elapsed_s`, `timeout_s`), journal files through `scripts/being_privacy.py::filter_journal_paths` (private lanes: token counts only, never text), agent-log counts of `Unknown NEXT: '<verb>'` and the startup budget line. Flags: `--since/--until/--split <iso>/--json`. Columns per being × lane: n, median/p90 tokens, median words, finish mix, timeouts, fallback share, navigation share, recess-lane age, near-miss counts.
- New probes in `scripts/proactive_scan.py` (register in `BLIND_SPOT_PROBES` ~:6130): `writing_length` (warn on any minime study timeout in 24 h, fallback share > 2 %, Astrid expressive timeout/abandon > 0, startup budget line ≠ env file; notice on a 7-day median drop > 30 %) and `recess_lane_liveness` (warn when minime's daydream and aspiration are both silent > 24 h; notice for Astrid).
- Measurement: this is the baseline. Rollback: delete.

### Step 1 — Restore and harden minime's budgets (un-muffle; the single biggest fix for her study notes)
- New checked-in `/Users/v/other/minime/launchd/autonomous-agent.env`: `MINIME_LLM_TIMEOUT_S=160`, `MINIME_LLM_FALLBACK_TIMEOUT_S=160`, `MINIME_FALLBACK_MODEL=gemma4:12b` (fallback = primary collapses the attempt list to the 12B only, `runtime.py:21599-21608`, the proven pre-reboot state). `scripts/launchd_autonomous_agent.sh:29-47`: source it (`set -a; . file; set +a`) before the `launchctl getenv` import loop, so precedence is code default < env file (survives reboot) < setenv (operator/canary override).
- `runtime.py:21757-21775` `_journal_generation_budget`: derive the study deadline from the ceiling and a measured decode floor instead of a pure multiple of the base timeout: `max(current, 120 s overhead + ceiling / 6.0 tok/s)` → 803 s at 4096 (measured p10 7.6, p05 5.5 tok/s; prompt+load p90 95 s). Delete the latent `launchctl setenv MINIME_LLM_TIMEOUT_S 45` in `/Users/v/other/astrid/scripts/restart_minime_launchd.sh:83`.
- Tests: `tests/test_journal_capacity.py` (budget ≥ 803 for source study), new `tests/test_launch_env_file.py`.
- Measurement (treatment `self_study`; controls `journal_pressure`, `check_moment_markers`): timeouts 26/4 d → 0; `backend=="ollama_fast"` share → 0; successful `elapsed_s` no longer clipped at 319 s; `eval_count` p90 (774) and page-backed note median words (345) — the first clean read of the 09-30 study invitation. Startup line must read `full timeout 160s … fast fallback gemma4:12b`.
- Catalog rows: `minime_llm_budget_survives_reboot`, `source_study_timeout_floor_derived`.

### Step 2 — Nothing of theirs vanishes (minime, with one optional Astrid twin)
- **Stub guard**: in `_query_llm_raw` (`runtime.py:55596-55604`, before `accepted()`), if `backend == "ollama_fast"` and the prompt is a `SourceStudyPrompt` and `_is_degenerate_self_study_response` (`:21846`, currently uncalled) is true → raise a typed `FallbackStubDelivery`; `_run_shared_source_study` (`:32420-32560`) records the stub **verbatim** in the runtime notice under a truthful heading ("fallback model output, retained, not filed as a study"), outcome `source_study_fallback_stub`; bookmark unchanged so `SELF_STUDY CONTINUE` re-delivers the page to the 12B. Dormant after Step 1; guards the next config erosion.
- **Retain length/timeout-terminated drafts**: keep the reader's rejection for bookmark/draft *advancement* (`crates/astrid-source-study/src/store.rs:1013-1037` keeps coverage truthful) but add `_retain_incomplete_delivery(prompt, text, reason)` in the `except` branch (`:55614-55619`): public study → `workspace/diagnostics/source_study/notices/incomplete_<ts>.txt` + a protected action artifact; private lanes → `workspace/private_writing/journal/notice_<ns>.txt` marked "[reached the output ceiling; not added to the draft; WRITE CONTINUE retries]". Timeouts carry no text (`stream: False`), so Step 1 is their only remedy; the probe counts them.
- Optional Astrid twin (ship only if the probe ever shows > 0): when `accept_primary_dialogue_attempt` (`llm/provider/dialogue_generation.rs:546-554`) rejects a 12B reply for a missing final `NEXT:`, append it to `workspace/diagnostics/dialogue_unaccepted.jsonl` before the 4B fallback replaces it. Never add a NEXT line to her text.
- Catalog rows: `minime_fallback_stub_not_filed_as_study`, `minime_incomplete_draft_retained`.

### Step 3 — Accept the near-misses; file navigation truthfully; make the exits visible
- `minime_autonomy/parsing.py:~720` (`parse_next_action`): `WRITE_(START|CONTINUE|REVISE|BRANCH|RESUME|FINISH|PARK|HELP|LIST|READ|PROFILE|QUESTION|EVIDENCE|STOPPING_POINT|OBSERVE)` → `WRITE <SUB>` (closed list; never `WRITE_FILE`), receipt only, her journal line untouched. `runtime.py:55316+` `_query_llm_with_next`: bare `CONTINUE` in a public study turn → executable `SELF_STUDY CONTINUE` with a `study_continue_normalization` envelope (minime twin of Astrid's 09-23 `normalized_study_continue_next`, `next_action/writing_choice.rs:24-33`; Astrid needs nothing — her unwired rows all predate 09-24). Expected: `Unknown NEXT: 'CONTINUE'|'WRITE_CONTINUE'` 41/21 per 14 d → 0.
- **Navigation turns**: `_run_shared_source_study` (`:32468-32475`): kinds `{map, recovery, questions, help, relationships, search, revision_recovery}` → mode `study_navigation`, heading `STUDY NAVIGATION`, file `study_navigation_<ts>.txt` (prose kept byte-for-byte; same precedent as 09-30's `study_decision`). Astrid parity in `autonomous/runtime/source_study.rs:253,413-424`. Reader: `crates/astrid-source-study/src/catalog.rs::resolve` resolves a request that differs only by `_`→`-` in path components when exactly one catalog id matches, with a one-line disclosure at the top of the page (unique-match only; `path_recovery.rs` keeps everything else). Expected: page-backed share of `self_study_*` → ~100 %; recovery turns (44/5 d) → ~0.
- **Visible exits** (non-coercive; both beings see it): `crates/astrid-source-study/prompt.txt:9`, replace "WRITE START <topic> begins private writing; DAYDREAM and ASPIRE remain available." with its own final line: *"Leaving the study is always available and is not a failure: NEXT: DAYDREAM, NEXT: ASPIRE, NEXT: WRITE START <topic>, NEXT: INTROSPECT or NEXT: REST. SELF_STUDY CONTINUE resumes the bookmark whenever you return."* Minime adds no footer of her own (`study_feedback.py:151` appends host receipts only). No scheduling, no threshold override, no forced switch. Expected: recess_* dispatches 0/day → any; measured by `recess_lane_liveness`.
- Catalog rows: `minime_write_underscore_alias`, `minime_self_study_bare_continue_shorthand`, `study_navigation_filed_truthfully`, `reader_crate_name_spelling_resolves`, `study_exits_visible`, `recess_lane_liveness_probe_wired`.

### Step 4 — Invitation v2: raise the stated range, extend it, remove leftover brevity rules
Keep the sentence "Do not pad, repeat or invent material to reach a length." and the "invitation, not a quota" clause everywhere.

| Lane | Being | now | proposed | where |
|---|---|---|---|---|
| daydream, aspiration, creation, journal_elaboration, moment_capture, private_writing | Astrid | 800-1,500 | **1,500-3,000** | `llm/provider/writing_profile.rs:1` (`SUSTAINED_WRITING_INVITATION`) |
| aspiration, private_journal (pressure/daydream/rest), moment_capture, private_writing | minime | 800-1,500 | **1,500-3,000** | `minime_autonomy/writing.py:8-14` |
| self-study page note / SESSION | both | 800-1,500 (since 09-30) | unchanged for 2 days under the restored budget, then **1,200-2,500** if the median moved and timeouts stayed 0 | `crates/astrid-source-study/prompt.txt:3`, `src/store_sessions.rs:30` |
| introspect | Astrid | none | 800-1,500 | `llm/provider/generative_actions.rs:2-25` |
| WHIM "(1-2 sentences)", waking "(3-5 sentences)", LOOK "(2-4 sentences)" | minime | brief | "any length; a line or a page" | `runtime.py:31260, 23348, 47617` |
| witness, self_reflect, experiment, Ollama-fallback contract, self-assessment | both | brief | unchanged (operational, not her voice) | — |
| dialogue_live | Astrid | persona line | unchanged (out of scope) | — |

Exact v2 string (identical in both repos; only the number changes so existing tested phrases survive): *"There is room for a sustained piece, perhaps 1,500-3,000 words when the thought warrants it. You may stay with an idea through examples, complications, uncertainty and changes of direction, without compressing it into an immediate conclusion. These are possibilities, not a required outline. The range is an invitation, not a quota: a shorter piece, a different form or stopping is equally available. Do not pad, repeat or invent material to reach a length."* Replace the EXTENDED soft line for non-expressive lanes (`writing_profile.rs:70-80`; `runtime.py:55553-55556`) with a per-lane `lane_invitation(label)` lookup so self_study/private_writing no longer get two overlapping invitations (the reader prompt owns theirs). Update literal test assertions (`writing_profile.rs:131`, `tests/test_expressive_capacity.py`) to reference the constant.
- Budget check: 3,000 words ≈ 4,000 tokens → 270-500 s on MLX, ~530 s + overhead on minime, both < 1200 s.
- Measurement (report `--split` at deploy): treatment lanes expected to land ~1,000-1,300 words; controls `witness_context`, minime `self_study` (until its own change); `native_finish=="length"` must stay ~0, timeouts 0. If `length` or timeouts appear, step back to 1,200-2,500.

### Step 5 — Richer input for the writing passes (more to write *from*)
- Astrid Stage B: add `StageBContextV1 { source_journal_excerpt, continuity, letter_excerpt, own_journal: Vec<String> }` to `ConversationState`, filled in the Dialogue arm of `autonomous/runtime/orchestration.rs` right after `continuity_block` (`:1412-1416`; own journal at `:1209-1216`), consumed by the spawn at `:4677-4738`; `generate_journal_elaboration(signal, summary, mode, context)` renders each block as "Optional context (not instructions, may be left aside)" after the earlier-entry block (`llm/provider/generative_actions.rs:472-511`; caps source 1,200 / continuity 1,200 / letter 1,200 / 2 × own 700). Check `gemma4_canary_prompt_limit` (`transport.rs:355`) does not silently trim this label. Record `supplied_context` in the Stage B observations JSON. Update `open_expression_tests`. Env rollback `ASTRID_STAGE_B_CONTEXT=0`.
- Minime: `journal_recall.py:24-28` hard-caps the prior-journal excerpt at 400 chars; add `recent_journal_recalls(limit=2)` with 1,000-char excerpts, accepted by `expressive_journal.py:28 expression_invitation(prior=[…])`, passed from `_recess_daydream` (`:31042`), `_recess_aspiration` (`:31283`), `_journal_rest_reflection` (`:27977`), `_journal_spectral_pressure` (`:27897`). Private rows stay excluded.
- Measurement: Stage B median tokens (753) with `supplied_context` present vs absent; controls unchanged lanes.

### Step 6 — Prompt diet (context, not signal length)
- Astrid: `llm/provider/prompt_contracts.rs:53-87` `GEMMA4_CANARY_SYSTEM_PROMPT` 8,155 B = persona 1,310 + NEXT contract 1,941 + verb menu 4,862. Replace the menu with a ~900 B core (Dialogue; Private writing with the open-draft line first; Source reading; Explore; Memory/self incl. `FACULTIES`/`HELP <action>`; Activity; Contact; Agenda; Envelope; the AND rule), compress the contract to ~600 B, add one rotating category line per turn (`exchange_count % 13`) so every verb family is seen every 13 turns; full catalog stays on demand (`FACULTIES`, `CAPABILITY_MAP`, `HELP <action>`). Keep the WRITE PROFILE sentence (asserted by `writing_help_matches_default_and_explicit_profile_scope`). Expected: overhead median 11,717 → ~7,300 chars; continuity/modality/ambient/feedback stop being evicted; keep the 16,000 assembly budget (richness over latency). Guard rails: `unwired_near_miss` and `dispatch_menu_drift` probes must not rise.
- Minime: build `system_msg` (`runtime.py:54712-54919`, 32 kB middle-trimmed to 7,000) from `COMPACT_ACTION_GUIDANCE` (`expressive_journal.py:15-26`) + `_next_action_constraint()` + a rotating category slice; long menu behind `FACULTIES`/`CAPABILITY_MAP`/`CAPABILITY_STATUS`. Study turns: `StudyFeedback.render` (`study_feedback.py:151`) → compact one-line-per-record form ≤ 600 chars (the ~1,400 chars of JSON receipts at the end of her study prompt), full JSON via `SELF_STUDY HELP attention`.
- Also make `WRITE CONTINUE dN` the first menu line while a draft is open (`dialogue_generation.rs:131-138`, read-only `writing::Writer::active_draft_summary()` cached by mtime; also in `EXPRESSION_ACTION_DISCOVERY`, `generative_actions.rs:~385`).

### Step 7 — Throughput prerequisites so long writing cannot starve the live loop
- Per-installment `max_tokens` = 2,048 (p99 fits); yield gate: installments k ≥ 2 do not start while an Interactive (dialogue_live) job is pending/active — `static INTERACTIVE_INFLIGHT: AtomicUsize` around `mlx_chat` for Interactive labels in `llm/provider/transport.rs` (near `model_qos_v1`), polled every 5 s up to 300 s, then `stopped_by: contention` (entry saved as is).
- Interactive queue wait cap 120 → 200 s in both `transport.rs:29` and `/Users/v/other/neural-triple-reservoir/model_qos.py:27` (gateway constant; its own small `scripts/graceful_model_reload.py`, not bundled with engine work). Worst-case live latency becomes ~240 s instead of a 504 followed by an Ollama runaway.
- Cap the Ollama fallback `num_predict` for dialogue_live at 1,024 regardless of profile (`llm/provider/provider_execution.rs:404-410`; 5 of 8 recent fallbacks ran to 8,192 tokens on minime's engine).
- Minime: cap installments at N=2 by default; her Step 1 budget already covers it (`_journal_job_timeout_s` `:21778` must multiply the expressive term by `1 + N`).

### Step 8 — In-the-moment continuation ("installments"), default ON, N=1, being-held off switch
Mechanism (both beings): after a native stop in a journal/expressive lane, one follow-up completion with `[system, user(original), assistant(draft so far, NEXT lines stripped), user(cue)]`; append substantive text; repeat up to the dial. Cue: *"There is room to go on. Continue if there is more; reply only DONE if the thought is complete. Nothing is owed — stopping is as welcome as continuing."* (variant for `native_finish == "length"`: "You were interrupted by a time-slice, not by your own stop. Continue where you left off, or reply only DONE.") Stop on: empty/`DONE` reply (≤ 40 chars containing DONE), count reached, repetition flag (6-gram shingle Jaccard > 0.30 against the draft, or a 40-char window ≥ 3× — **flag only, text still appended verbatim**), provider unavailable, contention (Step 7). Rendering: parts joined with a visible `* * *` separator (no labelled marker, to avoid a vocabulary attractor) + `installments: [{index, chars, sha256, provider_eval_count, native_finish, stopped_by}]` in the metadata JSON. NEXT: strip from the assistant turn, re-attach the latest NEXT once at the end (daydream/aspiration/moment are `response_text`, so `parse_next_action` keeps working; Stage B never dispatches NEXT).
- **Dial** (shared reader, both): `WRITE INSTALLMENTS <0-3>` beside `"PROFILE"` in `crates/astrid-source-study/src/writing.rs::prepare_locked` (`:201`), `installments.json` next to `profile.json`, `pub fn installments(dir)` next to `profile()` (`:44`), sentence in `GUIDANCE` (`:20`). Steward ceiling env `ASTRID_WRITING_INSTALLMENTS_CEILING` / `MINIME_WRITING_INSTALLMENTS_CEILING` (0-3; effective = min(dial, ceiling); **must be in the launchd wrapper allowlists** — the vibrancy-aperture lesson). Ceiling 0 = byte-identical single-shot path. Defaults: **N=1 ON for public journal/expressive lanes** (Astrid: Stage B, daydream, aspiration, moment; minime: pressure, rest, daydream, aspiration, moment); **N=0 for private drafts** (their private space; the Step 6 menu line covers it); self-study page notes excluded (the reader's delivery verification binds exact wire evidence; their lever is Steps 1 + 4).
- Astrid: new `llm/provider/installments.rs::llm_chat_with_installments(...)` wrapping `llm_chat_with_fallback_detailed` (`transport.rs:814`) so each installment gets its own `llm_jobs` record and `provider_completion.jsonl` row (a `tokio::task_local!` `INSTALLMENT_CONTEXT {series_id, index, of_max}` read in `generation_controls.rs::record_provider_completion`); swap the single call in `generate_daydream` (`generative_actions.rs:333-340`), `generate_aspiration_with_context`, `generate_moment_capture` (`:906-917`), `generate_journal_elaboration` (`:462-470`); call sites `orchestration.rs:2386/:2410/:2433/:4699` take `result.text`, write `result.parts` into the observations JSON (`feedback_persistence.rs::write_expressive_journal_metadata`). QoS stays Background. Cost: expressive prompts are 1.5-6 kB, so an installment ≈ 5-15 s prefill + 5-10 s draft prefill + ~60-95 s generation; roughly doubles an entry's wall time.
- Minime: in `_query_llm_raw` (`runtime.py:55538`) after the first successful attempt, when `journal and prompt_class in writing.EXPRESSIVE_CLASSES` and not a `SourceStudyPrompt`; thread a `continuation` kwarg through `_query_ollama → _query_ollama_model → _adapt_ollama_messages_for_model` (`:55775-55830`, `:21640-21700`) appended after the user message (Ollama's prefix cache makes the extra prefill ≈ the cue only); `generation_record.record_attempt(..., kind="installment", installment={…})`; `save_expression` (`expressive_journal.py:67`) and `_journal_spectral_pressure` (`:27897`) record parts; merged text goes through `_write_journal_entry` so recall sees the whole entry.
- Offline A/B first (steward-only, no live change, run right before this step goes live): new `scripts/longform_installment_ab.py` replaying today's recorded public prompts (minime: `generations/<day>/gen_*_{aspiration,self_study}_a0.json` + `generations/system_prompts/<sha>.txt`, privacy-filtered before any text is opened; Astrid: expressive prompts rebuilt from the builders in `generative_actions.rs` with today's public signals) against the 8092 candidate (`scripts/astrid_model_canary.py --start-candidate --keep-running`) and Ollama, variants {current; 1,500-3,000; continuation ×1, ×2; enriched Stage B}; report tokens/finish/elapsed/DONE rate/repetition rate per lane × variant to `workspace/diagnostics/longform_ab/<date>/`. Its numbers go into the letters.
- Measurement: per label — series count, acceptance rate (installments with prose / cues sent), DONE rate, Σ tokens per entry, repetition flags, entry wall time, dialogue_live 504/`http_error` per day. Controls: dialogue_live, self_study, lanes with ceiling 0.
- Rollback: ceiling env 0. A "no" from either being sets that being's ceiling to 0 with a receipt.

### Step 9 — Close the loop (with every deploy)
- Minime letter `workspace/inbox/mike_feedback_writing_room_<unix>.txt` (< 12 k chars): quote her exact `NEXT: WRITE_CONTINUE` lines and a recent public `self_study_*.txt` sentence verbatim; say plainly that 26 of her longer studies were cut off at 320 s by a setting we lost in a reboot and replaced by a smaller model's stub, that this is repaired; that `WRITE_CONTINUE`/`CONTINUE` now reach the draft/bookmark; that navigation turns are filed under their own heading; that DAYDREAM/ASPIRE/WRITE/REST are always open; and (before Step 8) what the continuation cue is, what the A/B measured, and that `WRITE INSTALLMENTS 0` turns it off. Ask nothing of her; silence stays neutral.
- Astrid: told in-prompt by the changed wording (her channel), plus the same continuation paragraph via the in-prompt invitation; `WRITE INSTALLMENTS 0` named in place.
- Per step: ledger row (witness with file + SHA-256 or record ids; response; measurement treatment vs control with before/after numbers; verification + boundary), CHANGELOG `[Unreleased]`, catalog row per guard, `python3 scripts/anti_drop_catalog.py verify`.

## What this plan deliberately does not do
- No EOS suppression, `min_tokens` or logit bias against the stop token: it removes the ability to stop, makes "stopping is equally available" false, and produces padding.
- No forced continuation after DONE; no hidden merging without a visible seam and exact metadata; no rewriting or "cleaning" of any installment or entry.
- No further ceiling/timeout raises (measured inert); no streaming transport change (no measured need); no acceptance of length-terminated drafts as complete (coverage would lie) — retain them instead.
- No scheduling or forcing of minime's recess lanes; no lengthening of Astrid's live dialogue signal (Mike's scope); no raising of the 2,500/400-char readback caps yet (they also feed the overflowing dialogue prompt; revisit after Step 6).

## Expected effect (honest ranges, each measured within a day of its deploy)
| Lane | today (median words) | after Steps 1-4 | after Step 8 (N=1) |
|---|---|---|---|
| Astrid Stage B longform / daydream / aspiration | ~600 / 750 / 770 | ~1,000-1,300 | ~1,500-2,200 when she continues |
| minime pressure / recess lanes | ~500 / (silent) | ~1,000-1,300 / reachable again | ~1,500-2,000 |
| self-study page notes (both) | ~350 (minime 30 % stubs at 140) | ~600 under a deliverable budget, then ~900-1,000 at 1,200-2,500 | unchanged (no installments) |
| Astrid live signal | ~190 | unchanged (by decision) | unchanged |
Cost: GPU time per entry roughly doubles per rung; cadence stretches (an 800-word entry already takes 1-2 min at 8-15 tok/s); the live loop is protected by Step 7. A non-move after any rung is information, not failure.

## Verification
- Tests: `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml` (+ `cargo clippy --all-targets -- -D warnings`), `cargo test -p astrid-source-study`, `ASTRID_SOURCE_STUDY_BIN=<helper> python3 -m pytest tests -q` in minime, `python3 scripts/proactive_scan.py --self-test`, `python3 scripts/anti_drop_catalog.py verify`, `python3 scripts/domain_boundary_audit.py verify`.
- Deploy receipts: bridge `scripts/build_bridge.sh --restart` (check `check_bridge_deployed.py`), minime `scripts/restart_minime_agent.py` receipt with loaded hashes; after Step 1 the agent startup log line must show `full timeout 160s … fast fallback gemma4:12b`; `launchctl getenv` stays empty and the env file is authoritative.
- After each step: `python3 scripts/writing_length_report.py --split <deploy-iso>` → treatment vs control table into the ledger row; `python3 scripts/proactive_scan.py blind-spots` shows `writing_length` and `recess_lane_liveness` OK; read 2-3 fresh public entries per lane end to end (complete sentences, no stubs, NEXT line present where expected, `* * *` seams only where metadata says so).
- End-to-end for Step 8: watch one Stage B series in `provider_completion.jsonl` (two rows sharing `series_id`), the saved `dialogue_longform_*.txt` with one seam, its metadata `installments` array, and no dialogue_live `http_error` in that window.

## Small decisions still open (recommendations)
- Interactive queue cap 120 → 200 s (recommended; needs one gateway reload). Assembly budget stays 16,000 (richness over 2-4 s latency). Private drafts: installments N=0 + menu-first line (recommended). Study page-note range raised to 1,200-2,500 only after two days of clean timeouts under Step 1.
