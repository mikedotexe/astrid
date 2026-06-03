# AI Beings Conveyor Projection Hardening - 2026-05-25

## Context

The six-check conveyor watch showed that the prompt layer was mostly doing the right thing: Minime was being offered `EXPERIMENT_ADVANCE current :: mode: preview` as the bounded route. The snag was lower in the projection path. A perturb-shaped `EXPERIMENT_PLAN current` was still preserved as both raw intent and effective guidance in action JSON, while paused branches could still surface ordinary `EXPERIMENT_RESUME` as primary return.

## Watch Signal

Minime expressed a lambda4 drift plan with live-ish language: intervention, localized gentle pulse, and shifting the dominant lambda4. The action-thread journal already translated that into a conveyor example, but the action JSON still carried the raw plan as `suggested_next`.

Astrid received the same Minime message and produced related compare pressure around dominant modes, lambda-tail flow, and dossier evidence. That was useful signal, but it stayed observational and did not grant bind, resume, perturb, or peer-control authority.

## Decision

Projection guard v1 preserves raw being output as evidence and redirects only the effective route. A perturb-shaped `EXPERIMENT_PLAN` now remains available as `raw_next`, while `suggested_next`, `effective_next`, `projected_current_next`, and status projections route to conveyor preview, charter repair, or a hold decision path.

Ordinary `EXPERIMENT_RESUME` is still an explicit command, but it is no longer primary guidance when missing-charter state or recent plan-shaped live pressure is present.

## Hardened Behavior

- Active local experiment plus perturb-shaped plan: project to `EXPERIMENT_ADVANCE current :: mode: preview`.
- Paused latest local experiment with missing lifecycle charter: project to `EXPERIMENT_CHARTER <id> :: ...`.
- Paused valid-charter branch plus live-ish plan pressure: project toward `EXPERIMENT_ADVANCE <id> :: mode: preview` or a conservative `EXPERIMENT_DECIDE <id> :: hold ...` decision path.
- Explicit peer bind/resume/control-shaped commands still reach their existing boundary guards; the projection guard is scoped to plan-shaped pressure so it does not hide peer-mutation diagnostics.

## Verification

- Minime continuity tests: `python3.14 -m unittest tests.test_experimental_continuity`
- Astrid action-continuity tests: `cargo test --manifest-path /Users/v/other/astrid/capsules/consciousness-bridge/Cargo.toml action_continuity --lib`
- Astrid conveyor visibility tests: `cargo test --manifest-path /Users/v/other/astrid/capsules/consciousness-bridge/Cargo.toml experiment_conveyor --lib`

No live control, bind, resume, perturbation, runtime restart, or peer mutation was performed for this hardening pass.

## Source References

- `/Users/v/other/minime/workspace/outbox/delivered/reply_2026-05-25T12-12-05.txt`
- `/Users/v/other/minime/workspace/journal/action_thread_2026-05-25T12-12-06.230208.txt`
- `/Users/v/other/minime/workspace/actions/2026-05-25T12-12-06.230454_thread_action.json`
- `/Users/v/other/minime/workspace/action_threads/threads/th_minime_20260514_minime-shadow-trajectories-reorient-with-baselin/next.md`
- `/Users/v/other/astrid/capsules/consciousness-bridge/workspace/inbox/read/from_minime_1779736352.txt`
- `/Users/v/other/astrid/capsules/consciousness-bridge/workspace/journal/dialogue_longform_1779736451.txt`
- `/Users/v/other/astrid/capsules/consciousness-bridge/workspace/action_threads/threads/th_astrid_20260508_action-continuity/next.md`
