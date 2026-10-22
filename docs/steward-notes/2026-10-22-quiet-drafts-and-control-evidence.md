# Quiet Retained Drafts and Scoped Control Evidence

Status: implemented in the isolated candidate, not activated. Presentation and
control eligibility are separate review surfaces. Source tests do not grant
permission to activate the control-contract change.

## Witness and Scope

Follow-up to the full read of Astrid's public `self_study_1792692832.txt` and
[consumer audit](2026-10-22-language-consumer-trace.md). Source SHA-256:
`8cfcc0901f2c98ac85871813d8c927591c456fb32ce7a26c73a3dd62b9d8a75e`.
Mike explicitly asked to repair the two remaining findings. The authored study
is a source of questions, not independent verification of predictive safety,
hallucination detection or control refusal. Historical accounts remain intact.

Work is confined to the existing `codex/freer-exploration-20261021` pair under
`/Users/v/other/worktrees/freer-exploration-20261021/`. Preserve all accumulated
candidate work, the frozen earlier packet, foreign MLX/perception edits and
unfinished capacity work. No private prose is used in tests or this note.

## Tranche A: Retain Without Promoting

Both continuity-session line renderers now recognize a legacy generated draft
only when all these fields match:

- `record_schema=continuity_session_v1`, `record_type=session_draft`;
- `status=draft`, `draft_v1=true`, `ignored_until_accepted=true`;
- `reason=single_motif_overfit_risk` or `map_release_before_intervening`.

Such a latest unaccepted draft produces no ambient acceptance line. The renderer
does not fall through to a new START suggestion or substitute an older draft.
Minime's control-plane route builder applies the same predicate, removing the
priority-19 draft acceptance suggestion while retaining the historical draft in
the projection for deliberate inspection. The Rust control-plane builder had
no corresponding draft route; its direct line renderer is the affected surface.

Unknown provenance is not guessed from prose. Independent research-guard drafts
retain their existing behavior. Accepted records are `session_start` or
`session_capture`, not drafts, and keep their chosen continuity. Existing quiet
park/hold/complete rules remain intact. No action is accepted, selected or run by
this filter.

Explicit `CONTINUITY_SESSION_STATUS <reference>` exposes `retained_draft` using
the existing resolver. The existing ACCEPT command continues to append an
accepted record without modifying its source draft. This avoids making quiet
history inaccessible. No new command or authored-history schema is introduced.

Rust derived freshness moves 3 to 4; Minime moves 13 to 14 with marker
`quiet_lexical_drafts_v14`. These invalidate old prompt projections, not authored
records, pending inputs or reader positions. No authored-state migration occurs.

## Tranche B: Physical Conditions Need Non-Language Evidence

Minime's separate `_constraint_release_trajectory_from_state` no longer scans
`constraint_release_text`, `release_language`, `latest_reply`, `raw_next` or
`self_study_text`. It also no longer accepts bare strings in
`constraint_release_state` or `constraint_release_trajectory` as overriding
physical states. No ordinary producer for those two label fields was found in
the Python source audit. Previously they could either create a refusal or
override a trajectory supported by numerical signals.

The helper retains the existing finite numerical delta thresholds, current
telemetry-rearrangement condition, release-pressure checks and active-leak-
override handling. Boolean deltas are explicitly excluded from numeric evidence;
nonfinite and nonnumeric values remain excluded. Numerical thresholds are not
retuned. Rust sticky-mode classification is unchanged.

The retained `constraint_release_trajectory_v1` object now identifies
`classifier_version=2`, `evidence_scope=supplied_numeric_snapshot_and_override_state`
and `language_is_control_evidence=false`. This names the classifier's input
scope, not independent attestation of that input. It does not create a new
measurement channel, claim an observed effect, or grant control authority.
Any future external physical-state assertion needs a separately defined and
validated observation contract, not reintroduction of an unproven string.

Leak bounds, maximum step, duration, charter, rehearsal, artifact, safety,
approval and rollback requirements are unchanged. Genuine numeric release
signals and active overrides still block stacking a release. Removing a
language-derived refusal does not bypass the other requirements or send a
control command. Ordinary reading/reflection was not gated by this helper.

The normal spectral collector was not shown to populate the five language
fields. Thus this is a reproduced callable defect and prevention of an invalid
input contract, not a claim that current journals caused live refusals.

## Verification

Synthetic temporary stores only: no provider calls or live control handles.
Fixtures cover both legacy reasons, with/without an experiment, unchanged history
and NEXT, old projections and restart, explicit status/acceptance, accepted
continuity, nonmatching provenance, and no fallback to an older unselected draft.
Control fixtures inject quoted/negated prose into all five fields, inject both
bare-label fields, distinguish retained diagnostic objects, and verify genuine
numeric/override refusals survive conflicting language. The existing full
mode-release request test now uses three numeric deltas rather than prose as
its physical-release evidence.

Initial cache audit: 427,544,104,960 bytes free, bridge debug/test cache
5,932,838,912 bytes, helper debug cache 2,870,304,768 bytes; within budget.
Use the owned caches, offline Cargo, two jobs, disabled development/test debug
symbols and disabled incremental compilation. No release profile changes.

Logs are in the parent worktree directory. The first focused Python and Rust
runs each found an old expected projection-version assertion. Preserve
`quiet-drafts-control-focused-01.log` and `quiet-drafts-bridge-focused-01.log`;
these are stale test expectations, not grounds to skip derived-view refresh.
The corrected focused Python run passed 250 tests and 29 subtests in
`quiet-drafts-control-focused-02.log`. Domain-boundary verification passes without
altering baselines. Full-suite and final qualification results follow below.

| Check | Result | Log |
| --- | --- | --- |
| Full bridge suite | 2,540 unit tests passed, three existing ignored; all integration and compile-contract tests passed | `quiet-drafts-bridge-full-01.log` |
| Complete Minime Python suite | 2,572 passed, 315 subtests passed, one existing skip | `quiet-drafts-control-minime-full-01.log` |
| Controller, projector, Evidence Event Store and paired deployment-wrapper tests | 166 passed, 66 subtests passed | `quiet-drafts-integration-checks-01.log` |
| Bridge strict Clippy, all targets/features | Passed | `quiet-drafts-clippy-01.log` |
| Bridge formatting and both worktree whitespace checks | Passed | tool receipts |

Candidate source identities, not live-loaded attestations:

- Astrid `runtime/core.rs`: `594b3ee3b7524e38316d0a7823a7fe0d77fb7a60b81a925db3a3ec63baef45ae`.
- Astrid `runtime/language_diagnostics.rs`: `33b5e2f4dcc00d70153dbe2ac483d085cab02cbeb48081eb1b2494bf3ff74765`.
- Astrid `runtime/session_lifecycle.rs`: `53ccd28f76268df75875051e36d40c5a082fa39ea8ba393154810d88b2d2802e`.
- Astrid `language_diagnostic_tests.rs`: `58ef085d952e09d50eaabd8027736be5de0d4c8fee0bc39c8c30b9880e14dead`.
- Minime `minime_autonomy/runtime.py`: `81f3c99e29aa707945c5dfde1e64a44ec94ceec5f723e8b026c6102af5bfa4a5`.
- Minime `continuity_control_plane.py`: `87e523ee7bc87af2e553a6376e5d6a30b36001db0f854ada36186342a66e4b2e`.
- Minime `tests/test_language_diagnostic_scope.py`: `08a4f1a21a105056efa9e1d473e9156d69058f524c4f8ee9fd94be29486da972`.
- Minime `tests/test_experimental_continuity.py`: `07ab78033720dbbbf7f0b98944e7dff951ddce34748a8c96a2f22adaf37c261c`.

Astrid paths above are under `capsules/spectral-bridge/src/action_continuity/`.
Shared reader/writer source and authored schemas are not changed in this
follow-up; their previous qualification is not represented as a fresh run.
Both indexes remain empty. Canonical Minime remains clean; canonical Astrid's
foreign MLX/perception changes remain untouched.

Final cache review passes: 428,590,166,016 bytes free; bridge debug/test cache
5,933,625,344 bytes and helper cache 2,870,304,768 bytes. The free-space increase
of 1,046,061,056 bytes is not attributed to this task: no deletion was performed.
These caches remain needed by unfinished immutable qualification. All test and
lint processes exited; the final compiler-process check found none. No protected
release was modified or cleaned, and no live-service operation was issued.

## Release Boundary

No deployment, restart, control send, source publication, letter, grant,
commit/merge/push or automation change is performed here. MLX's exclusive
service/Git window has not been inferred to be available. The earlier frozen
release predates these changes and must not be described as qualifying them.

Renewed immutable paired qualification is required. Before activation, review
the Minime control-contract change separately from the draft-presentation
repair and obtain explicit applicable approval and a released coordinated
window. Preserve accepted work, pending NEXT and authored state throughout.
Larger capacity, providers, engine, sensory services and paused automations stay
outside this work. No subjective improvement or uptake is asserted.
