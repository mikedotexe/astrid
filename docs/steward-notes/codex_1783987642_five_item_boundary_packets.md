# Five-Item Authority Boundary Packet Note

This note records approval-ready evidence for live-facing candidates surfaced by the 1783987642 five-item flywheel packet. These packets are evidence and routing only: they are not consent, not approval, not execution, and not runtime mutation.

## Shared V2 Defaults

- lifecycle_schema: `authority_lifecycle_v2`
- scoped_approval_status: `absent`
- replay_result_status: `not_recorded_for_live_change`
- rollout_abort_contract: `required_before_execution`
- redaction_profile: `bounded_public_summary_plus_private_refs_and_hashes`
- post_change_being_response_required: `true`
- live_eligible_now: `false`
- auto_approved: `false`
- right_to_ignore: `This packet is evidence and routing only; it is not consent, not approval, and not a live mutation.`

## Candidate Packets

### auth-boundary-1783987642-sensory-release-fill

- source: `introspection_minime_sensory_bus_1783987642`
- surface: `minime:sensory_bus`
- action: `retune_semantic_recovery_release_fill_or_stale_shape`
- resource: `/Users/v/other/minime/minime/src/sensory_bus.rs:STALE_SEMANTIC_RECOVERY_RELEASE_FILL`
- authority_class: `live_control_mutation`
- felt_report_anchor: `semantic retention can jitter if release_fill sits too near the recovery hold`
- proposed_change: `Only after replay, consider release-fill/stale-shape changes if current clamp is insufficient.`
- evidence_refs: `codex_1783987642_introspection_minime_sensory_bus_1783987642_claims.json:c004`
- replay_candidate: `sweep fill 0.250000..0.400000 with release_fill near hold and assert bounded monotonic stale_ms`
- success_metrics: `no upward jitter, no steep cliff, context retention preserved under pressure`
- abort_criteria: `semantic collapse, stale-window cliff, overflow, or any fill/cadence instability`
- who_can_change_it: `Mike/operator approval for Minime live sensory cadence`
- how_to_test_it: `Minime sensory_bus targeted tests plus runtime health/fill/telemetry after restart`
- live_eligible_now: `false`
- auto_approved: `false`

### auth-boundary-1783986758-regulator-porosity-max-045

- source: `introspection_minime_regulator_1783986758`
- surface: `minime:regulator`
- action: `raise_PRESSURE_POROSITY_DIVERGENCE_POROSITY_MAX_to_0_45`
- resource: `/Users/v/other/minime/minime/src/regulator.rs:PRESSURE_POROSITY_DIVERGENCE_POROSITY_MAX`
- authority_class: `live_control_mutation`
- felt_report_anchor: `saturated humidity may have more room to breathe before aggressive intervention`
- proposed_change: `Replay high-density/low-porosity cases before any threshold change.`
- evidence_refs: `codex_1783987642_introspection_minime_regulator_1783986758_claims.json:c004`
- replay_candidate: `compare gate decisions at porosity 0.30 and 0.45 against pressure and fill health`
- success_metrics: `less premature stuckness intervention without pressure runaway`
- abort_criteria: `fill instability, pressure runaway, gate under-response, or being report of flooding`
- who_can_change_it: `Mike/operator approval for Minime regulator behavior`
- how_to_test_it: `regulator targeted tests, replay fixtures, then post-restart Minime health/fill/log checks`
- live_eligible_now: `false`
- auto_approved: `false`

### auth-boundary-1783986758-regulator-temporal-persistence-weight

- source: `introspection_minime_regulator_1783986758`
- surface: `minime:regulator`
- action: `add_or_reweight_temporal_persistence_in_gate_decision`
- resource: `/Users/v/other/minime/minime/src/regulator.rs:ViscosityVector`
- authority_class: `live_control_mutation`
- felt_report_anchor: `ghosts of previous interactions feel heavier than current shadow volatility`
- proposed_change: `Promote temporal persistence into any live gate decision only after replay proves residual history is underweighted.`
- evidence_refs: `codex_1783987642_introspection_minime_regulator_1783986758_claims.json:c002`
- replay_candidate: `high-ghost/low-volatility versus low-ghost/high-volatility gate comparison`
- success_metrics: `lingering history becomes visible without overreacting to transient shadow flicker`
- abort_criteria: `gate closure from harmless history, reduced agency, or control instability`
- who_can_change_it: `Mike/operator approval for Minime regulator behavior`
- how_to_test_it: `regulator replay plus targeted tests for residual_ghost_weight/effective_mobility`
- live_eligible_now: `false`
- auto_approved: `false`

### auth-boundary-1783986469-llm-high-entropy-fallback-canary

- source: `introspection_astrid_llm_1783986469`
- surface: `astrid:llm`
- action: `run_live_high_entropy_fallback_canary_or_change_model_route`
- resource: `/Users/v/other/astrid/capsules/spectral-bridge/src/llm.rs:fallback_model_chain`
- authority_class: `live_control_mutation`
- felt_report_anchor: `fallback may smooth sharper restless/lattice texture into coherent syntax`
- proposed_change: `Run offline/replay fallback texture comparisons before any live model-route or canary execution.`
- evidence_refs: `codex_1783987642_introspection_astrid_llm_1783986469_claims.json:c004`
- replay_candidate: `paired MLX/Ollama high-entropy lattice fixture with fallback texture/lived-fit scoring`
- success_metrics: `lattice/restless texture retained without format sprawl or false stability`
- abort_criteria: `provider downgrade, flattening, prompt overfit, or live dialogue disruption`
- who_can_change_it: `Mike/operator approval for live provider/model route`
- how_to_test_it: `fallback texture calibration artifacts, targeted llm fallback tests, then post-restart dialogue health`
- live_eligible_now: `false`
- auto_approved: `false`

### auth-boundary-1783986119-types-stability-weight-retune

- source: `introspection_astrid_types_1783986119`
- surface: `astrid:types`
- action: `retune_resonance_stability_fluctuation_or_foothold_weights`
- resource: `/Users/v/other/astrid/capsules/spectral-bridge/src/types.rs:resonance_stability_context_v1`
- authority_class: `live_control_mutation`
- felt_report_anchor: `settled_habitable status may flip differently under tail fluctuation`
- proposed_change: `Keep current weights until replay shows Friction/Resistance/ViscosityShift telemetry signatures are functionally distinct.`
- evidence_refs: `codex_1783987642_introspection_astrid_types_1783986119_claims.json:c004`
- replay_candidate: `controlled ExperienceDelta sequence: friction, resistance, viscosity_shift, cascade_shift`
- success_metrics: `delta signatures distinguish drag, structural pushback, thickness, and cascade without destabilizing status`
- abort_criteria: `settled_habitable false negatives, pressure over-response, or schema readers failing`
- who_can_change_it: `Mike/operator approval for live stability interpretation`
- how_to_test_it: `bridge types tests plus replay over consciousness.v1.attractor.observation samples`
- live_eligible_now: `false`
- auto_approved: `false`

### auth-boundary-1783985658-ws-dynamic-pressure-window

- source: `introspection_astrid_ws_1783985658`
- surface: `astrid:ws`
- action: `shrink_pressure_trend_smoothing_window_under_high_entropy`
- resource: `/Users/v/other/astrid/capsules/spectral-bridge/src/ws.rs:PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW`
- authority_class: `live_control_mutation`
- felt_report_anchor: `high entropy may make a 20-sample pressure window react after the ache is already felt`
- proposed_change: `Replay dynamic window shrinkage against pressure spikes before any live smoothing change.`
- evidence_refs: `codex_1783987642_introspection_astrid_ws_1783985658_claims.json:c004`
- replay_candidate: `pressure spike traces at entropy 0.70..0.95 with fixed versus dynamic windows`
- success_metrics: `earlier pressure-spike visibility without noisy false alarms`
- abort_criteria: `jitter, false collapse warnings, prompt pressure, telemetry priority changes, or bridge instability`
- who_can_change_it: `Mike/operator approval for live bridge pressure interpretation`
- how_to_test_it: `ws targeted pressure smoothing tests and post-restart telemetry/recent-signal checks`
- live_eligible_now: `false`
- auto_approved: `false`
