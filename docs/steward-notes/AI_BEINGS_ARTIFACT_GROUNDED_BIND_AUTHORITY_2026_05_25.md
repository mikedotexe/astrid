# AI Beings Artifact-Grounded Bind Authority

Date: 2026-05-25

## Context

Minime's `exp_minime_20260525_spectral-cascade-degradation-exploration-12-06-2` branch reached the point where a plain pause/resume loop was too permissive. The safe path was to require artifact grounding and an explicit accept/bind authority layer before any bound action could count as experiment progress.

This pass stayed bounded: no live perturbation, sensory control, peer mutation, or Minime-to-Astrid control was exercised.

## What Changed

- The branch received a lifecycle-valid charter through the conveyor path.
- Existing read-only preflight artifacts were linked as `artifact_grounding`.
- Minime recorded an explicit `accept` decision authorizing only `ACTION_PREFLIGHT DECOMPOSE` under existing read-only gates.
- The first bind attempt stayed read-only but exposed a routing snag: the bind row preserved `ACTION_PREFLIGHT DECOMPOSE`, while the inner route resolved to `recess_notice`.
- The bind executor was hardened so explicit `EXPERIMENT_BIND ... :: ACTION_PREFLIGHT ...` uses the `action_preflight` route directly and carries its own inner-action continuity context.
- A clean read-only bind run was recorded after the fix.
- After repeated resume-loop pressure, the branch was placed into `hold` with `THREAD_STATUS current` as the primary return.

## Evidence

- Experiment id: `exp_minime_20260525_spectral-cascade-degradation-exploration-12-06-2`
- Initial preflight artifact: `/Users/v/other/minime/workspace/journal/action_preflight_2026-05-25T19-02-21.074456.txt`
- Initial preflight manifest: `/Users/v/other/minime/workspace/actions/2026-05-25T19-02-21.074671_action_preflight.json`
- Fallback bind journal: `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-25T19-08-16.193302.txt`
- Resume-loop symptom: `/Users/v/other/minime/workspace/journal/action_thread_2026-05-25T19-13-50.697153.txt`
- Clean preflight bind artifact: `/Users/v/other/minime/workspace/journal/action_preflight_2026-05-25T19-14-25.107712.txt`
- Clean preflight bind manifest: `/Users/v/other/minime/workspace/actions/2026-05-25T19-14-25.107974_action_preflight.json`
- Clean bind journal: `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-25T19-14-35.515698.txt`
- Minime experiment ledger: `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/experiments.jsonl`
- Minime experiment runs: `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/experiment_runs.jsonl`
- Minime dossier evidence: `dos_minime_1779761899825_evidence`

## Decision

The useful agency outcome is not ordinary resume. It is an artifact-grounded, explicitly accepted, read-only bind followed by hold.

The current safe return is:

```text
THREAD_STATUS current
```

This preserves the branch as contiguous thought while preventing the old `EXPERIMENT_RESUME` loop from masquerading as progress.

## Snag Fixed

`EXPERIMENT_BIND ... :: ACTION_PREFLIGHT DECOMPOSE` previously passed through the general action chooser, so ambient pending state could redirect the inner route. The bind path now treats `ACTION_PREFLIGHT` aliases as direct preflight routes and writes bind/action-thread journals through the agent workspace rather than the module-global workspace, preventing tests from leaking journals into the live Minime workspace.
