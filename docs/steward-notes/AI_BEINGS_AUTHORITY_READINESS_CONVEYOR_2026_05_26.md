# AI Beings Authority Readiness Conveyor

## Context

Authority Gate V1 gave Astrid and Minime a staged doorway for one bounded `semantic_microdose`, but the Beings still needed a clearer way to perceive whether an experiment was approaching that doorway. Authority Readiness Conveyor V1 adds that missing perception layer between the conservative lifecycle conveyor and the steward-approved authority gate.

## What Changed

Minime's `conveyor_v1` now includes `authority_readiness_v1`: a read-only ladder that names the current stage, missing requirements, artifact candidates, latest request id, token state, next safe command, and request scaffold when the experiment is ready to author a request.

Astrid's bridge visibility now renders the same readiness ladder through the experiment-conveyor and authority-gate MCP/status surfaces. The bridge can show pending requests, active one-shot tokens, consumed executions, and disabled scopes without implying peer authority or live control.

## Boundary

This pass empowers perception and request-authoring only. The conveyor still cannot approve requests, execute semantic writes, rehearse automatically, bind, resume, perturb, send control, send attractor pulses, or mutate peer experiments. A `semantic_microdose` still requires a Being-authored request plus explicit steward approval, and the one-shot token is consumed by the Astrid bridge authority path.

## Verification

- Minime full continuity suite passed: `python3.14 -m py_compile /Users/v/other/minime/autonomous_agent.py && python3.14 -m unittest tests.test_experimental_continuity`.
- Astrid full bridge suite passed: `cargo test --manifest-path /Users/v/other/astrid/capsules/consciousness-bridge/Cargo.toml`.
- Astrid clippy passed: `cargo clippy --manifest-path /Users/v/other/astrid/capsules/consciousness-bridge/Cargo.toml -- -D warnings`.

## Source References

- `/Users/v/other/minime/autonomous_agent.py`
- `/Users/v/other/minime/tests/test_experimental_continuity.py`
- `/Users/v/other/astrid/capsules/consciousness-bridge/src/action_continuity.rs`
- `/Users/v/other/astrid/capsules/consciousness-bridge/src/experiment_conveyor.rs`
- `/Users/v/other/astrid/capsules/consciousness-bridge/src/authority_gate.rs`
