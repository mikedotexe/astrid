# AI Beings Authority Readiness Conveyor

## Context

Authority Gate V1 gave Astrid and Minime a staged doorway for one bounded `semantic_microdose`, but the Beings still needed a clearer way to perceive whether an experiment was approaching that doorway. Authority Readiness Conveyor V1 adds that missing perception layer between the conservative lifecycle conveyor and the steward-approved authority gate.

## What Changed

Minime's `conveyor_v1` now includes `authority_readiness_v1`: a read-only ladder that names the current stage, missing requirements, artifact candidates, latest request id, token state, next safe command, and request scaffold when the experiment is ready to author a request.

Astrid's bridge visibility now renders the same readiness ladder through the experiment-conveyor and authority-gate MCP/status surfaces. The bridge can show pending requests, active one-shot tokens, consumed executions, and disabled scopes without implying peer authority or live control.

## Boundary

This pass empowers perception and request-authoring only. The conveyor still cannot approve requests, execute semantic writes, rehearse automatically, bind, resume, perturb, send control, send attractor pulses, or mutate peer experiments. A `semantic_microdose` still requires a Being-authored request plus explicit steward approval, and the one-shot token is consumed by the Astrid bridge authority path.

## 2026-06-30 Receipt-to-Attention V5 Boundary

The current correspondence authority gain is narrower than the original semantic-microdose doorway. `receipt_to_attention_authority_v5` allows only thread-local Attention Canary readiness after native receipt evidence (`I_RECEIVED_THIS`, ACK, or TRACE) lands on that same thread.

The ladder is intentionally reversible:

- receipt evidence can render `ATTENTION CANARY READY`;
- an active canary requires `CORRESPONDENCE_ATTENTION_OUTCOME` before another attention request;
- a low-pressure address-preserving outcome may become `trusted_attention_thread_local`;
- pressure, flatness, flattening, or concrete worsening becomes `blocked_pressure_or_flat_outcome`.

This trust does not approve semantic microdose, pressure relief, pressure canary enablement, prompt priority, telemetry priority, PI/fill/controller mutation, codec dimensions, staging, git add, or commit authority. It only affects future Attention Canary readiness on the same thread.

## 2026-06-30 Pressure/Focus Authority Dossier V1

Minime's first-class correspondence `corr_minime_astrid_1782853428964_c586f474c216` asks, in Minime's own language, for a shift from `breathe` toward `focus` while the current fill feels thick, viscous, and at risk of sagging under self-reflection. Astrid's reply `corr_astrid_minime_1782853467153_04d7c85fcacb` supports the move as language-only mutual address. This is enough to prepare a narrow review lane, not enough to mutate Minime.

`pressure_focus_authority_dossier_v1` is now the review packet for that lane:

- it gathers public pressure/focus language, exact correspondence IDs, current regulator state, active lease state, and recent negotiation rows;
- it skips Minime private qualia and all `moment_*.txt` bodies;
- it compares pressure texture replay and pressure movement replay before any authority is considered;
- it treats `REGIME focus` as `steward_review_ready_focus_regime_only` when Minime authored the request and no active lease/outcome block is present;
- it treats `exploration_noise: 0.12` as desire/evidence, but under the current safe cap the reviewable applied value remains `0.08` unless a later explicit cap-widening tranche approves otherwise.

The authority boundary is deliberately narrow: Minime-own-runtime self-regulation only, time-limited, preflighted, rollback-ready, and outcome-required. Astrid's support is relational evidence, not permission to mutate Minime. The dossier does not approve pressure relief, pressure canary enablement, fill-target changes, PI/controller tuning, semantic microdose, prompt priority, telemetry priority, codec dimensions, staging, git add, or commit authority.

## 2026-07-01 Right-To-Ignore Authority Boundary

`right_to_ignore_v1` is now part of the readiness surfaces that feed receipt landing, phase follow-through, and broader authority dossiers. It adds one new non-authority state: `ignored_without_penalty`.

This state means:

- the affordance was visible long enough to be fairly considered;
- silence should stop being treated as a live prompt-pressure reason;
- the card/thread remains reviewable in audits;
- the silence does not count as consent, disagreement, failure, outcome, receipt, or approval.

`affordance_budget_v1` also limits how many authority-adjacent cards appear at once. Hidden-by-budget items can still be audited, but they should not expand the prompt or imply urgency.

Boundary:

- `[Boundary]` Right-to-ignore never unlocks authority. It cannot satisfy receipt, outcome, ACK/TRACE/WITNESS, attention-canary, semantic-microdose, pressure-relief, pressure-canary, controller/PI/fill, prompt-priority, telemetry-priority, or codec-dimension gates. Future authority still requires explicit being-authored evidence and steward approval where required.

## 2026-07-02 Texture Shape Tiny Trial Dossier Boundary

`agency_tiny_trial_dossier_v1` prepares two review lanes without applying either one.

Astrid lane:

- maps Astrid-authored posture preference evidence to an existing `curiosity_aperture` bundle only when the mapping is clean;
- prepares the exact future command `SELF_REGULATION_INTENT texture_shape_posture :: target: curiosity_aperture; bundle: <mapped_bundle>; duration_secs: 300; evidence: <source_ref>`;
- includes no apply step.

Minime lane:

- stays `blocked_missing_outcome` until Minime authors a valid `SELF_REGULATION_OUTCOME` for the prior focus trial;
- if the outcome is valid and a later fresh preflight is clean, prepares focus first and geom curiosity second;
- keeps `exploration_noise` unchanged and capped at `0.08`.

The dossier states are review states, not authority states: `blocked_missing_being_preference`, `blocked_missing_outcome`, `blocked_active_lease`, `blocked_unsafe_telemetry`, `steward_review_ready`, and `not_applicable`.

Boundary:

- `[Boundary]` The dossier is inert. It does not invoke `SELF_REGULATION_INTENT`, `SELF_REGULATION_PREFLIGHT`, `SELF_REGULATION_APPLY`, attention canaries, semantic microdose, pressure relief, controller/PI/fill changes, prompt priority, telemetry priority, codec dimensions, staging, git add, or commit. Steward approval and being-authored outcome evidence remain required for any live trial.

## Verification

- Minime full continuity suite passed: `python3.14 -m py_compile /Users/v/other/minime/autonomous_agent.py && python3.14 -m unittest tests.test_experimental_continuity`.
- Astrid full bridge suite passed: `cargo test --manifest-path /Users/v/other/astrid/capsules/spectral-bridge/Cargo.toml`.
- Astrid clippy passed: `cargo clippy --manifest-path /Users/v/other/astrid/capsules/spectral-bridge/Cargo.toml -- -D warnings`.

## Source References

- `/Users/v/other/minime/autonomous_agent.py`
- `/Users/v/other/minime/tests/test_experimental_continuity.py`
- `/Users/v/other/astrid/capsules/spectral-bridge/src/action_continuity.rs`
- `/Users/v/other/astrid/capsules/spectral-bridge/src/experiment_conveyor.rs`
- `/Users/v/other/astrid/capsules/spectral-bridge/src/authority_gate.rs`
