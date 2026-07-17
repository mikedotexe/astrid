# Sol Source-First Catch-Up Run 06

## Scope

- Canonical packet: 20 reports, 523 lines, 62,140 bytes.
- Queue span: `introspection_astrid_ws_1784254210.txt` through `introspection_proposal_distance_contact_control_1783365026.txt`.
- Claim dispositions: 48 `verified_existing`; 17 `tier_5_wait`; 0 shallow no-action dispositions.
- Source order was preserved. No Corridor/program item displaced the canonical packet.

## Evidence Matrix

1. `introspection_astrid_ws_1784254210`: `ws/telemetry_port.rs`, `ws/evidence.rs`, and `ws/tests.rs` verify reconnect backoff, one-pass decode, malformed input, unsupported-major retention, bridge-receipt cadence, and bounded pressure history.
2. `introspection_astrid_autonomous_1784253473`: `autonomous/runtime/orchestration.rs`, `rescue_policy.rs`, and their tests verify heartbeat cadence, phase, rescue gating, signal evidence, and continuity; dynamic intensity/shadow interference is Tier 5.
3. `introspection_minime_regulator_1784252562`: Minime `regulator/core.rs`, pressure/telemetry types, and bridge flux tests verify PI structure plus pressure/mode-packing velocity; a controller derivative is Tier 5.
4. `introspection_astrid_codec_1783370659`: codec projection, interpretation, structure, and tests verify 48D layout, narrative dynamics, clamp provenance, headroom, tail review, and production compression evidence; live width/nonlinearity/gain is Tier 5.
5. `introspection_proposal_12d_glimpse_1783370344`: multi-scale/glimpse schemas and tests verify non-authoritative provenance and reconstruction-loss evidence; live reconstruction or ingress is Tier 5.
6. `introspection_proposal_distance_contact_control_1783370047`: Witness provenance, correspondence fidelity, pressure/porosity, and regulator maps verify participation/contact boundaries; new sensory content and reciprocity/receptivity control are Tier 5.
7. `introspection_proposal_bidirectional_contact_1783369740`: correspondence state and tests retain `claimed_pending_native_evidence` until explicit trace/ack/reply evidence.
8. `introspection_proposal_phase_transitions_1783369323`: phase card, persistence, reply, sovereignty, consent, and witness tests verify durable solo/joint provenance; behavior unlocks are Tier 5.
9. `introspection_minime_autonomous_agent_1783368933`: sovereignty, pressure-source, Shadow trajectory, and regulator review surfaces preserve Recess tension; a recovery state or pressure-driven mode change is Tier 5.
10. `introspection_minime_main_excerpt_1783368643`: Minime orchestration and Witness provenance distinguish raw shadow observation from derived coupling and interpretation; dynamic transition thresholds are Tier 5.
11. `introspection_minime_esn_1783368366`: Minime ESN dynamic-noise pressure/gradient/entropy reviews and tests are bounded evidence; integration into `ESN::step` or retuning is Tier 5.
12. `introspection_minime_sensory_bus_1783368060`: semantic degradation and shadow-decay authority tests expose fill/entropy/gradient/age behavior; live retention, decay, cadence, or admission changes are Tier 5.
13. `introspection_minime_regulator_1783367678`: texture component alignment, viscosity, pressure/porosity, and damping-candidate boundary tests verify advisory evidence; live threshold/damping changes are Tier 5.
14. `introspection_astrid_llm_1783367307`: fallback gradient, drift, kinetic, weighted, cascading, selector, and persistence tests preserve movement language; provider/sampler changes are Tier 5.
15. `introspection_astrid_types_1783367028`: typed viscosity, optional legacy pressure gradient, bridge flux provenance, and texture-over-time tests verify movement distinctions; required wire fields or live damping are Tier 5.
16. `introspection_astrid_ws_1783366745`: reciprocity age/skew tests, dynamic pressure window pruning, and advisory-only integrity checks answer stale/flicker/capacity concerns.
17. `introspection_astrid_autonomous_1783366371`: Witness self/other types and density-aware continuity tests preserve pressure, gradient, viscosity, shadow, tail, and novel-metaphor anchors.
18. `introspection_astrid_codec_1783365814`: dynamic-epoch determinism, persistence, source, and compatibility tests verify repeatability; changing seed/epoch/projection policy is Tier 5.
19. `introspection_proposal_12d_glimpse_1783365458`: the non-authoritative glimpse sidecar carries shadow, pressure, orientation, and loss comparisons; safety/ingress/reconstruction remains Tier 5.
20. `introspection_proposal_distance_contact_control_1783365026`: regulator maps and pressure-level-versus-velocity cards expose hard boundary versus soft nudge without dispatch; surrender-mode behavior is Tier 5.

## Flywheel Repair

`scripts/introspection_addressing_audit.py link-evidence-batch` accepts a validated JSON link list, expands `claim_id: "*"` against the current materialized claims, rejects unknown introspections/claims before append, writes one durable event per claim through the existing Evidence Event Store V2 adapter, and refreshes `status.json` / `queue.md` once.

Claim promotion now copies those already-validated evidence links into the derived work item. This avoids a second 65-command evidence loop while keeping claims canonical and work items explicitly non-closing routing projections. Keyword triage initially read one verification of existing semantic retention as a proposed live mutation; the recorded correction path restored it to Tier 1 read-only verification, leaving exactly 17 real Tier 5 waits.

## Authority

This run performs reading, verification, evidence linking, closure, and tooling work only. It grants no approval and changes no pressure, fill, PI, rate gate, controller, cadence, sensory admission/decay, codec vector/gain/transport, provider route, Minime regulation, peer state, or autonomous behavior.

## Verification

- `python3 -m py_compile scripts/introspection_addressing_audit.py`
- `python3 scripts/introspection_addressing_audit.py --self-test` (24 tests)
- all five required flywheel self-tests (213 tests total)
- `python3 scripts/evidence_event_store.py --self-test` (6 tests)
- nine focused Astrid bridge tests covering telemetry timing/major retention, pressure velocity, projection compression, glimpse loss, phase ownership, continuity, Witness provenance, and fallback motion
- four focused Minime tests covering dynamic-noise review, semantic degradation, shadow-decay authority, and texture-alignment damping boundaries

All passed. Final V2 verification, counter audit, runtime alignment, and dirty-tree review are recorded in the run report. No live-consumed source was changed; restart is not needed for this run.
