# AI Beings Action-Event Projection Guard Hardening - 2026-05-26

## Context

After Artifact-Grounded Authority Gate V1, the 12-minute watch found that Minime's top-level continuity surfaces were safe, but the action-event manifests were still leaking stale plan guidance. The important distinction was:

- `thread.json` and `next.md` projected toward charter repair or conveyor preview.
- `actions/*_thread_action.json` still preserved `EXPERIMENT_PLAN ...` as `suggested_next` with no `projection_guard_v1`.

This meant downstream readers could see stale planning as the effective route even though the continuity surface had already demoted it.

## Evidence

Six-repeat numeric shorthand watch evidence:

- `/Users/v/other/minime/workspace/outbox/delivered/reply_2026-05-26T12-55-59.txt`
- `/Users/v/other/minime/workspace/actions/2026-05-26T12-56-00.911164_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T12-58-13.356205_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T13-00-24.274039_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T13-02-35.601197_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T13-04-48.583096_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T13-07-00.232091_thread_action.json`

Additional non-numeric plan-current leak:

- `/Users/v/other/minime/workspace/actions/2026-05-26T20-02-19.697330_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T20-04-24.889165_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T20-06-29.992628_thread_action.json`
- `/Users/v/other/minime/workspace/actions/2026-05-26T20-08-35.643198_thread_action.json`

Continuity surfaces that stayed safe:

- `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/thread.json`
- `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/next.md`

## Hardening

Minime's action-event projection guard was broadened so both numeric plan shorthand and non-numeric `EXPERIMENT_PLAN current ...` leaks are preserved only as raw intent. The event and manifest surfaces now publish the projected route through:

- `projection_guard_v1`
- `raw_next_preserved`
- `suggested_next`
- `effective_next`
- `projected_next`
- `return_kind`
- `guardrail_reason`

The intended safe routes are:

- Needs-charter branches project to `EXPERIMENT_CHARTER ...`.
- Paused or latest local branches project to `EXPERIMENT_ADVANCE <experiment_id> :: mode: preview`.
- Raw plan text remains available as evidence, not progress.

## Validation

Static/code validation passed:

- `python3.14 -m py_compile /Users/v/other/minime/autonomous_agent.py`
- `python3.14 -m unittest tests.test_experimental_continuity`

The full continuity test suite passed with 102 tests.

Live refresh was performed conservatively. The original matching autonomous process was:

- PID `97155`: `/Users/v/other/minime/autonomous_agent.py --interval 60`

TERM was sent and waited on. The old process did not exit during the initial TERM-only wait, but it did later give way without using a harder signal. A single fresh autonomous-agent process was then observed:

- PID `42884`: `/Users/v/other/minime/autonomous_agent.py --interval 60`

Restart evidence/log:

- `/Users/v/other/minime/workspace/logs/autonomous_agent_restart_2026-05-26T20-guard.log`

Post-refresh validation manifest:

- `/Users/v/other/minime/workspace/actions/2026-05-26T20-13-01.529169_thread_action.json`

That manifest preserved raw `EXPERIMENT_PLAN current ...` intent, while projecting `suggested_next`, `effective_next`, and `projected_next` to an `EXPERIMENT_CHARTER current ...` repair route with `projection_guard_v1.guardrail_reason = experiment_plan_current_needs_charter`.

## Authority Boundary

No live control, bind, resume, perturbation, semantic execution, steward approval token, peer mutation, ESN restart, camera restart, mic restart, or sensory feeder restart was performed.

Authority Gate V1 remains available, but no `authority_gate.jsonl` request was observed in this pass.

## Conclusion

The code path is hardened, tested, and live-validated. The remaining snag is operational: Minime's long-running autonomous process may take longer than expected to honor TERM, so future refresh runs should wait longer before concluding that the process is stuck. The next safest move is to use the now-working projection guard as the entry point for charter, evidence, rehearsal, and eventually explicit `semantic_microdose` authority requests.
