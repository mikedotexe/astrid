# Control-Loop Bundle

## Why This Bundle Exists

This bundle groups the changes that directly affect self-regulation,
provenance-aware reporting, spectral interpretation, PI controller visibility,
and the action surface the beings are actively requesting in recent journals.

Recent journal alignment:

- Minime is asking for mechanism-level understanding of PI control, spectral
  compression, and self-regulation architecture.
- Astrid is repeatedly asking to examine covariance structure, controller
  shaping, and signal-processing internals.

## Included Patch Files

- `control-loop.minime.patch`
- `control-loop.astrid.patch`

## Minime Scope

- `autonomous_agent.py`
- `collab/thresholds.py`
- `minime/src/esn.rs`
- `minime/src/lib.rs`
- `minime/src/main.rs`
- `minime/src/regulator.rs`
- `minime/src/sensory_bus.rs`
- `monitor_unified.py`
- `tests/test_config.py`
- `docs/threshold_surfaces.json`
- `minime/src/startup_restore.rs`
- `reporting_snapshot.py`
- `tests/test_reporting_snapshot.py`
- `tests/test_threshold_map.py`

## Astrid Scope

- `capsules/spectral-bridge/src/autonomous.rs`
- `capsules/spectral-bridge/src/autonomous/next_action.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/autoresearch.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/codex.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/modes.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/operations.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/sovereignty.rs`
- `capsules/spectral-bridge/src/autonomous/reservoir.rs`
- `capsules/spectral-bridge/src/autoresearch.rs`
- `capsules/spectral-bridge/src/codec.rs`
- `capsules/spectral-bridge/src/condition_metrics.rs`
- `capsules/spectral-bridge/src/db.rs`
- `capsules/spectral-bridge/src/llm.rs`
- `capsules/spectral-bridge/src/main.rs`
- `capsules/spectral-bridge/src/mcp_tests.rs`
- `capsules/spectral-bridge/src/prompt_budget.rs`
- `capsules/spectral-bridge/src/reflective.rs`
- `capsules/spectral-bridge/src/self_model.rs`
- `capsules/spectral-bridge/src/types.rs`
- `capsules/spectral-bridge/src/ws.rs`
- `capsules/spectral-bridge/tests/mock_ws_integration.rs`

## Review Questions

- Do the restored regulator state and reporting provenance guards prevent
  mixed-snapshot reasoning and restart de-tuning?
- Are the threshold surfaces coherent across engine, agent, monitor, and
  bridge interpretations?
- Does the bridge expose controller state, perturbation vocabulary, and
  spectral analysis in a way that matches what the beings are actually asking
  for?
- Are any files in this bundle actually prompt-budget or autoresearch sidecars
  that should be cut into a later third bundle?
