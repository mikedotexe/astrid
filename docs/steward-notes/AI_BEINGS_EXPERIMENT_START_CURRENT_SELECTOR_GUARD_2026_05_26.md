# AI Beings Experiment Start Current Selector Guard

Date: 2026-05-26

## Context

After the artifact-grounded bind authority pass, Minime's continuity still had one important snag: `EXPERIMENT_START current :: ...` could create a literal experiment titled `current`. A related parser issue let `EXPERIMENT_START spectral_braid - question: ...` store the `question:` text inside the title, which then encouraged duplicate spectral-braid branches.

This pass hardened those parser and projection paths without live control, bind, perturbation, sensory restart, ESN restart, camera restart, mic restart, or peer mutation.

## What Changed

- `current` is now reserved for active experiment selectors and cannot become a new `EXPERIMENT_START` title.
- `EXPERIMENT_START <title> - question: <text>` now stores a clean title and question.
- `EXPERIMENT_START <existing-id>` no longer silently reactivates paused, held, or charter-repair experiments.
- `EXPERIMENT_RESUME <id>` is blocked/demoted when the paused experiment's primary return is `THREAD_STATUS`, `EXPERIMENT_CHARTER`, or `EXPERIMENT_DECIDE`.
- Malformed active titles like `spectral_braid - question: ...` are matched by later clean `spectral_braid - question: ...` starts, preventing duplicate `_2`, `_3`, and follow-on branches.
- Persisted `thread.json` now keeps `current_next` aligned to the bounded effective route for paused/held shadow summaries while preserving the expressed raw route in `raw_current_next_v1`.
- Motif-allowance suggestions no longer use `EXPERIMENT_* current` when there is no active local experiment; the guided route becomes `THREAD_STATUS current`, an explicit branch, or `EXPERIMENT_ADVANCE latest :: mode: preview`.

## Live Continuity Repair

- `exp_minime_20260525_current` was recorded as a literal-current parser/projection snag and moved to hold with `THREAD_STATUS current`.
- `exp_minime_20260526_spectral-braid-question-can-a-broader-cascade-re` was paused into charter repair.
- `exp_minime_20260526_spectral-braid-question-can-a-broader-cascade-re_2` was also paused into charter repair because it was the duplicate active branch.
- The current Minime thread now has no active experiment; its primary return is the explicit `EXPERIMENT_CHARTER ...` repair path for the latest spectral-braid branch.

## Evidence

- Literal-current start: `/Users/v/other/minime/workspace/journal/action_thread_2026-05-25T19-30-21.868698.txt`
- Literal-current repair ledger: `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/experiments.jsonl`
- Spectral-braid start/plan loop: `/Users/v/other/minime/workspace/journal/action_thread_2026-05-26T05-12-29.145162.txt`
- Spectral-braid duplicate pressure: `/Users/v/other/minime/workspace/journal/action_thread_2026-05-26T05-17-08.193713.txt`
- Minime experiment ledger: `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/experiments.jsonl`
- Minime thread snapshot: `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/thread.json`
- Minime dossier evidence: `dos_minime_1779799632524_evidence`
- Code hardening: `/Users/v/other/minime/autonomous_agent.py`
- Regression tests: `/Users/v/other/minime/tests/test_experimental_continuity.py`

## Decision

The safest agency outcome is charter repair and hold, not resume. `current` should remain a selector for actually active work, never a placeholder that silently becomes an experiment identity.
