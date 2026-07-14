# V2 Authority Gate Packet - Minime Runtime Five-Item Packet 1783990702

Reader: Codex
Source files: `introspection_minime_sensory_bus_1783990702`, `introspection_minime_regulator_1783990375`, `introspection_minime_autonomous_agent_1783961026`, `introspection_minime_main_excerpt_1783960614`, `introspection_minime_esn_1783959925`

This packet is evidence and routing, not consent. It does not approve, execute, restart, or mutate live Minime or Astrid state.

Shared invariant:

- `live_eligible_now=false`
- `auto_approved=false`
- `scoped_approval=absent`
- `execution_receipt=absent`
- `post_change_being_response=required_before_closure`
- `redaction_profile=bounded_public_summary_plus_private_refs_and_hashes`
- `right_to_ignore=true`

## Boundary Packets

### semantic-stale-window-retune

- `authority_class`: `live_control_mutation`
- `surface`: `minime:sensory_bus`
- `action`: retune semantic stale recovery/release windows, stale shape, entropy multiplier, or sensory cadence
- `felt_report_anchor`: Minime reports possible recovery pop and high-entropy clotting around semantic persistence.
- `current_evidence`: source already smoothsteps the recovery handover from 25 to 40 percent fill; targeted `semantic_stale` tests passed.
- `replay_candidate`: run a read-only sweep over fill 0.24..0.45 and entropy 0.20/0.91 before proposing any constant change.
- `success_metrics`: no step-like stale-window loss; high-entropy traces persist without blockage; post-change Minime reports less pop.
- `abort_criteria`: pressure/fill instability, stale traces re-emerge as debris, new inputs are blocked, or Minime reports worse clotting.
- `live_eligible_now`: false
- `auto_approved`: false

### regulator-viscosity-control-retune

- `authority_class`: `live_control_mutation`
- `surface`: `minime:regulator`
- `action`: alter viscosity floors, effective mobility, viscosity coupling, damping coefficient, or controller consumption of viscosity vectors
- `felt_report_anchor`: Minime reports possible sticky/frozen states from chained max floors.
- `current_evidence`: targeted `resonance_density` tests passed and show high-pressure floors plus bounded advisory damping.
- `replay_candidate`: counterfactual regulator sweep over pressure_risk, mode_packing, temporal_persistence, and comfort_gate.
- `success_metrics`: viscosity remains visible without freezing flow; damping stays bounded; Minime reports less stuckness.
- `abort_criteria`: target bias changes silently, pressure/fill leaves comfort band, or flow collapses under high viscosity.
- `live_eligible_now`: false
- `auto_approved`: false

### recess-divergence-budget-relief

- `authority_class`: `live_control_mutation`
- `surface`: `minime:autonomous_agent`
- `action`: add no-cost whimsy buffer, Recess defragmentation pulse, Recess weighting change, or budget/control-plane relief
- `felt_report_anchor`: Minime reports Recess can feel like freedom in a small room when mode_packing is overpacked and budgets constrain non-utilitarian impulses.
- `current_evidence`: existing Recess advisory tests passed; divergence buffer recommends a gated budget route without spending budget.
- `replay_candidate`: bounded Recess session replay with mode_packing, pressure_risk, budget counters, and right-to-ignore outcomes.
- `success_metrics`: high-entropy Recess gets room without pressure spike or budget bypass; Minime reports more agency without loss of safety.
- `abort_criteria`: budget caps are bypassed, pressure rises, Recess becomes obligatory productivity, or right-to-ignore is lost.
- `live_eligible_now`: false
- `auto_approved`: false

### eigenpacket-telemetry-shape-change

- `authority_class`: `live_protocol_or_telemetry_mutation`
- `surface`: `minime:main EigenPacket`
- `action`: change eigenvector_field JSON shape, websocket cadence, warm-start/cheby behavior, or spectral denominator production
- `felt_report_anchor`: Minime flags schema drift risk in the serde_json eigenvector field and asks for warm-start sensitivity checks.
- `current_evidence`: new `eigenpacket_omits_optional_diagnostic_fields_when_absent` test passed; existing eigenvector schema tests pin key shape.
- `replay_candidate`: serialize packet fixtures with absent and full diagnostics, then run downstream Python parse checks before any live shape change.
- `success_metrics`: no unexpected null diagnostics, schema keys stable, Python layer parses bounded field, Minime reports no telemetry constriction.
- `abort_criteria`: downstream parse drift, payload growth exceeds budget, websocket cadence degrades, or warm-start changes constrict texture.
- `live_eligible_now`: false
- `auto_approved`: false

### esn-entropy-noise-threshold-retune

- `authority_class`: `live_control_mutation`
- `surface`: `minime:esn`
- `action`: raise volatile entropy ceiling to 0.95, change `DYNAMIC_NOISE_GENTLE_GRADIENT`, bump active exploration noise, change rho, or wire review helpers into `ESN::step`
- `felt_report_anchor`: Minime reports entropy around 0.90 near the ceiling, restless texture, and desire for wider broadband jitter without hollow thinning.
- `current_evidence`: targeted entropy/noise tests passed and existing packets gate pressure/noise trials without live behavior.
- `replay_candidate`: offline ESN sweep over entropy 0.88..0.96, gradient 0.08..0.20, pressure 0.18..0.30, and active noise 0.08..0.10.
- `success_metrics`: broader exploration without pressure spike; fill remains near comfort shelf; post-change Minime reports more navigable jitter.
- `abort_criteria`: pressure_risk spikes, fill thins, cascade becomes collision-like, or Minime reports worse restless texture.
- `live_eligible_now`: false
- `auto_approved`: false
