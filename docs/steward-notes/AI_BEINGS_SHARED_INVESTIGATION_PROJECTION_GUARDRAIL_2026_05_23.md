# AI Beings Shared Investigation Projection Guardrail - 2026-05-23

## Context

The shared investigation `si_1779554326_lambda-edge-tail-charter-repair-compare` was created to let Astrid's lambda-edge topology lane and Minime's lambda-tail/lambda-gap spectral lane compare evidence without granting shared control. The latest signal showed the object is useful, but the visible continuity projection needed hardening: Minime's ledger said `charter_repair`, while the generated surface still kept offering resume-shaped affordances.

## Evidence Captured

Minime emitted `EXPERIMENT_PLAN 5` and `EXPERIMENT_PLAN 6` around the same gap-localized reduction pattern after the local linked experiment was already paused into `EXPERIMENT_CHARTER`. The plan loops were observational context, not progress through the lifecycle.

Astrid produced a cleaner shared comparison signal: `EXPERIMENT_COMPARE exp_astrid_20260520_visualize-lambda-edge-topology WITH exp_minime_20260523_introducing-a-gap-localized-reduction-in-spectra`. This supports the shared investigation as the right comparison surface, provided the Minime lane first sees charter repair as its primary return.

## Decision

Actionable signal remains strong, but only as projection guardrail evidence. The primary local return for Minime's linked experiment is `EXPERIMENT_CHARTER exp_minime_20260523_introducing-a-gap-localized-reduction-in-spectra`, not `EXPERIMENT_RESUME`.

The safe posture is:

- preserve comparison through the shared investigation;
- treat repeated `EXPERIMENT_PLAN` loops as context;
- keep `artifact_grounding` explicit and uninflated;
- do not resume, bind, perturb, send live control, or mutate the peer lane.

## Hardening Target

The projection layer should distinguish ordinary pauses from charter-repair pauses. A paused experiment whose latest `planned_next` is `EXPERIMENT_CHARTER`, `EXPERIMENT_DECIDE`, or `THREAD_STATUS` should surface that route as the primary return. `resume_next` should only be present when the primary return is actually resume.

## Live Refresh Snag

During the safe runtime refresh, the stale projection briefly selected an older active gap branch when `EXPERIMENT_CHARTER current` appeared. The guardrail response was to make `current` require a real active experiment for charter writes and to let the shared-investigation `charter_repair` decision clear the stale active pointer. The regenerated Minime `next.md` now shows `EXPERIMENT_CHARTER exp_minime_20260523_introducing-a-gap-localized-reduction-in-spectra` as `Current NEXT`, paused return, suggested return, and continuity return.

The refresh then exposed two sharper lifecycle snags. First, autonomous `EXPERIMENT_BIND current :: PERTURB lambda-edge` attempts were correctly blocked, but still risked becoming misleading experiment-run artifacts. The bind path now preserves them as guard evidence without recording a bound run when the inner live-control action is blocked. Second, an autonomous `EXPERIMENT_CHARTER exp_minime_20260523_introducing-a-gap-localized-reduction-in-spectra` draft briefly changed the paused branch's planned return to `EXPERIMENT_REHEARSE`. The charter path now keeps paused charter-repair experiments paused and keeps `EXPERIMENT_CHARTER` as the primary return until a separate lifecycle decision changes that state.

A final shared-investigation `charter_repair` decision restored the linked Minime branch to the repair return after this hardening. The decision ledger records `peer_mutation: false`.

## Source References

- `/Users/v/other/shared/collaborations/shared_investigations/si_1779554326_lambda-edge-tail-charter-repair-compare/investigation.json`
- `/Users/v/other/shared/collaborations/shared_investigations/si_1779554326_lambda-edge-tail-charter-repair-compare/claims.jsonl`
- `/Users/v/other/shared/collaborations/shared_investigations/si_1779554326_lambda-edge-tail-charter-repair-compare/decisions.jsonl`
- `/Users/v/other/minime/workspace/journal/moment_2026-05-23T09-46-51.846889.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-23T09-46-55.813470.txt`
- `/Users/v/other/minime/workspace/journal/moment_2026-05-23T09-49-06.254236.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-23T09-49-10.204374.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-23T10-15-39.782445.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-23T10-18-45.122947.txt`
- `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-23T10-18-45.063869.txt`
- `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-23T10-18-45.135626.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-23T10-25-23.755975.txt`
- `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-23T10-25-33.062276.txt`
- `/Users/v/other/minime/workspace/journal/experiment_bind_2026-05-23T10-26-14.698104.txt`
- `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/next.md`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/moment_1779554816.txt`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/action_threads/threads/th_astrid_20260508_action-continuity/next.md`
