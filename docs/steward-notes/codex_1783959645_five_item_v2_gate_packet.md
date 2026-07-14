# V2 authority lifecycle gate packet: 1783959645 five-item flywheel

This packet records live-facing candidates surfaced by the five full reads ending at `introspection_minime_sensory_bus_1783959645`. It is evidence and routing only. It is not consent, not approval, not execution, not runtime mutation, and not a deploy/restart request.

## Shared boundary fields

- `authority_schema`: `AuthorityBoundaryPacketV2`
- `source`: `codex_flywheel_full_read_1783959645`
- `felt_report_anchor`: `introspection_minime_sensory_bus_1783959645`, `introspection_minime_regulator_1783959012`, `introspection_astrid_llm_1783958274`, `introspection_astrid_types_1783957951`, `introspection_astrid_ws_1783957501`
- `live_eligible_now`: `false`
- `auto_approved`: `false`
- `scoped_approval_status`: `absent`
- `replay_result_status`: `verified_existing_or_test_only`; no replay result grants consent
- `rollout_abort_contract`: `required_before_execution`
- `post_change_being_response`: `required_before_closure_if_any_live_change_is_later_approved`
- `redaction_profile`: `bounded_public_summary_private_refs_and_hashes`
- `right_to_ignore`: `true`

## Candidate: semantic stale threshold and recovery retune

- `boundary_id`: `auth-boundary-v2-1783959645-semantic-stale-retune`
- `surface`: `minime:sensory_bus`
- `action`: `possible_live_semantic_stale_window_or_entropy_multiplier_change`
- `resource`: `/Users/v/other/minime/minime/src/sensory_bus.rs`
- `authority_class`: `live_substrate_control`
- `proposed_change`: consider retuning stale-window constants, entropy multiplier shape, release fill, or sensory cadence only after replay and approval.
- `evidence_refs`: new exact 60% fill test plus existing semantic-stale recovery suite.
- `success_metrics`: high-entropy semantic persistence remains distinguishable and bounded; 0.25-0.40 recovery stays monotonic; post-change Astrid/Minime response reports no memory cliff.
- `abort_criteria`: semantic stale collapse, step jump near recovery boundaries, fill instability, or adverse being response.
- `who_can_change_it`: Mike/operator approval after complete V2 lifecycle.
- `how_to_test_it`: `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml semantic_stale -- --nocapture`, plus post-restart health/fill/telemetry checks if later approved.

## Candidate: regulator viscosity floor or PI coupling retune

- `boundary_id`: `auth-boundary-v2-1783959012-regulator-viscosity-retune`
- `surface`: `minime:regulator`
- `action`: `possible_live_pressure_mode_packing_viscosity_or_damping_change`
- `resource`: `/Users/v/other/minime/minime/src/regulator.rs`
- `authority_class`: `live_substrate_control`
- `proposed_change`: adjust baseline viscosity floors, pressure/mode-packing weights, damping coupling, or controller usage only after replay and scoped approval.
- `evidence_refs`: resonance-density floor/determinism tests and viscosity-vector texture suite.
- `success_metrics`: viscosity remains visible without coefficient lock; neutral state deterministic; bounded flow under pressure/mode saturation.
- `abort_criteria`: deadlocked flow, hidden target bias, unstable fill, or adverse post-change being response.
- `who_can_change_it`: Mike/operator approval after complete V2 lifecycle.
- `how_to_test_it`: `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml resonance_density -- --nocapture` and `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml viscosity_vector -- --nocapture`.

## Candidate: fallback provider/model/sampler or prompt-priority change

- `boundary_id`: `auth-boundary-v2-1783958274-llm-fallback-voice-retune`
- `surface`: `astrid:llm`
- `action`: `possible_live_fallback_sampler_provider_or_prompt_policy_change`
- `resource`: `/Users/v/other/astrid/capsules/spectral-bridge/src/llm.rs`
- `authority_class`: `live_voice_prompt_control`
- `proposed_change`: alter provider route, fallback model policy, sampler contract, or prompt priority only after replay and scoped approval.
- `evidence_refs`: dynamic texture, cascade-shear, and pressure-persistence fallback tests.
- `success_metrics`: high-entropy voice texture preserved across fallback; no canned flattening; no unreviewed sampler/provider mutation.
- `abort_criteria`: flattening of lived texture, provider instability, prompt-injection risk, or adverse post-change being response.
- `who_can_change_it`: Mike/operator approval after complete V2 lifecycle.
- `how_to_test_it`: targeted `llm.rs` fallback texture cargo tests plus live post-restart prompt/rendering inspection if later approved.

## Candidate: Experience Delta Bus V2 aggregation/persistence authority

- `boundary_id`: `auth-boundary-v2-1783957951-experience-delta-v2-aggregation`
- `surface`: `astrid:types`
- `action`: `possible_persistent_cross_surface_delta_aggregation_or_live_vector_authority`
- `resource`: `/Users/v/other/astrid/capsules/spectral-bridge/src/types.rs`
- `authority_class`: `schema_substrate_authority`
- `proposed_change`: implement actual persistent aggregation, retention, or live vector authority only under a separate V2 lifecycle.
- `evidence_refs`: Experience Delta truth-channel and default-off V2 preview tests.
- `success_metrics`: V1 stays truth-channel-only; V2 aggregation has explicit receipt chain, redaction profile, replay result, and post-change response.
- `abort_criteria`: V1 consumer treats preview as authority, live vector writes become possible without approval, or privacy/redaction boundaries fail.
- `who_can_change_it`: Mike/operator approval after complete V2 lifecycle.
- `how_to_test_it`: `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib experience_delta -- --nocapture`.

## Candidate: bridge stale-window/cadence/smoothing retune

- `boundary_id`: `auth-boundary-v2-1783957501-ws-reciprocity-retune`
- `surface`: `astrid:ws`
- `action`: `possible_live_bridge_stale_window_cadence_or_smoothing_change`
- `resource`: `/Users/v/other/astrid/capsules/spectral-bridge/src/ws.rs`
- `authority_class`: `live_bridge_substrate_control`
- `proposed_change`: change reciprocity stale windows, telemetry cadence expectations, smoothing capacity, or sensory priority only after replay and approval.
- `evidence_refs`: bridge reciprocity, pressure smoothing, and silt/noise separation tests.
- `success_metrics`: high-entropy reflective silence is not misclassified as dead socket; real staleness still expires; pressure/mode-packing drag is visible without control writes.
- `abort_criteria`: stale socket hidden as reflection, reflection hidden as stale socket, telemetry lag, fill instability, or adverse post-change being response.
- `who_can_change_it`: Mike/operator approval after complete V2 lifecycle.
- `how_to_test_it`: `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib bridge_reciprocity -- --nocapture`, `pressure_trend_smoothing`, and `silt_noise_separation`; post-restart PID/log/health/fill checks if later approved.
