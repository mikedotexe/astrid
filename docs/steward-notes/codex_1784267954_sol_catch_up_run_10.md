# Sol Source-First Catch-Up Run 10

- run_id: `codex_1784267954_sol_catch_up_run_10`
- authority: evidence and runtime-adjacent diagnostics only
- selected_packet_count: 20
- fully_processed_count: 20
- selected_packet_unprocessed: none
- canonical_start_remaining: 1052
- canonical_final_remaining: 1033
- gross_quality_guard_reduction: 20
- net_quality_guard_reduction: 19
- new_canonical_arrivals_during_run: 1
- restart_alignment: current

## Full-Read Packet

The packet was read fully from disk in canonical queue order:

1. `introspection_astrid_ws_1784267160.txt`
2. `introspection_astrid_autonomous_1784266777.txt`
3. `introspection_astrid_codec_1784266401.txt`
4. `introspection_minime_regulator_1784265926.txt`
5. `introspection_astrid_llm_1784265520.txt`
6. `introspection_astrid_ws_1784265017.txt`
7. `introspection_astrid_codec_1784264652.txt`
8. `introspection_minime_main_excerpt_1783341397.txt`
9. `introspection_minime_esn_1783339962.txt`
10. `introspection_minime_sensory_bus_1783339513.txt`
11. `introspection_minime_regulator_1783339139.txt`
12. `introspection_astrid_llm_1783338139.txt`
13. `introspection_astrid_types_1783337864.txt`
14. `introspection_astrid_ws_1783337379.txt`
15. `introspection_astrid_autonomous_1783336916.txt`
16. `introspection_astrid_codec_1783336630.txt`
17. `introspection_proposal_12d_glimpse_1783336346.txt`
18. `introspection_proposal_distance_contact_control_1783335883.txt`
19. `introspection_proposal_bidirectional_contact_1783335595.txt`
20. `introspection_proposal_phase_transitions_1783335310.txt`

Bounded summaries and all 73 claim records are in
`docs/steward-notes/codex_1784267954_run10_reads/`.

## Claim Dispositions

- implemented_now: 4
- verified_existing: 45
- needs_sandbox: 6
- tier_5_wait: 18
- tier_4_wait in selected packet: 0
- unsupported no-action dispositions: 0

All 20 reports have durable full-read records, claim dispositions, evidence
links, `addressed_change` closures, and right-to-ignore report/claim cards.
The run wrote 97 claim-evidence links, promoted 73 work items, closed 20
reports, and emitted 73 closure cards.

## Implemented Evidence

`telemetry_integration_health_v1` now separates the latest, EWMA, and maximum
pre-write pipeline duration, shared-state write-lock wait, and write-lock hold.
It explicitly records:

- `causal_attribution=not_established_by_timing_alone`
- `buffered_integration=false`
- `cadence_write=false`

The evidence is available in latest telemetry metadata and both bridge status
projections. No telemetry buffer, cadence, packet, controller, or control
behavior changed.

Dialogue quality regressions now pin Astrid's exact nine-symbol rejection and
prove that punctuation-rich reflective prose remains accepted.

The linked sandbox result exposed a flywheel lifecycle mismatch. Sandbox
correctly required `status=ready_for_sandbox`, but Corridor treated the durable
`runnable` capability bit as current executability. Corridor now:

- requires `ready_for_sandbox` before routing a runnable trial;
- preserves `result_recorded` packets as completed evidence;
- suppresses duplicate lab and generic source-prep steps for completed trials;
- sorts genuinely ready trials ahead of non-runnable lifecycle states.

After regeneration, `trial_8528bd6c523b93cb` has no runnable Corridor step.
The packet mix changed from 60 mislabeled runnable labs to 5 recorded results
and 55 approval-bound proposals.

## Sandbox

The directly relevant read-only trial was:

- trial: `trial_8528bd6c523b93cb`
- adapter: `shadow_loss_lattice_v1`
- classification: `lattice_transition_like`
- sample_count: 2
- lattice_language_hits: 6
- loss_language_hits: 0
- max_dispersal: 0.0
- min_norm_delta: unavailable
- result_sha256:
  `6d1b597c086e003f055bb0f7dabfbf7b20bc313301073991a7ce937c95e2a268`

This is a bounded texture classification, not a causal conclusion. It changed
no live state. One result card and five proposal cards were emitted. The five
remaining selected-packet observations are classified and carded but have no
currently executable adapter.

Final Sandbox state:

- total: 1072
- active: 1071
- ready: 147
- result_recorded: 88
- approval_required: 836
- closed: 1
- ready_runnable: 0
- proposal_cards: 103
- result_cards: 86
- live violations: 0
- Tier 4: 17
- Tier 5: 834

## Corridor And Escalator

Generic ready bundles were not run because they were unrelated to the current
packet and would have displaced source reading. The linked observation ran
through the Sandbox queue; Corridor was regenerated afterward and repaired
only because the completed result remained incorrectly runnable.

Final routing state:

- packets: 120
- leases: 35 total, 4 active, 31 imported evidence-only, 0 revoked
- queue steps: 187
- runnable non-live steps: 127
- programs: 118 active
- stored portfolios: 200
- current-program portfolios: 118
- patch bundles: 45
- source-prep proposals: 67
- reopened work items: 0
- self-observation requests: 60
- self-observation responses: 0
- hard violations: 0

The next runnable Corridor step is bounded
`request_scoped_self_observation` step
`08ad6f6a-16fd-54c6-a28f-e1f668189d67` for
`introspection_astrid_ws_1783337379`. It grants no approval and makes no live
work executable. There is no currently runnable Sandbox trial.

## Tests

Passed:

- focused telemetry-integration and dialogue quality tests;
- bridge `cargo test --lib`: 1523 tests;
- strict bridge Clippy;
- bridge formatting;
- Astrid/Minime protocol fixtures: 6 plus 2;
- authority typestate compile-fail suite: 3 UI cases;
- provenance typestate compile-fail suite: 4 UI cases;
- Python authority-state tests: 6;
- Agency Corridor self-test: 18;
- introspection addressing self-test: 29;
- sandbox queue self-test: 26;
- recent signal self-test: 38;
- proactive scan self-test: 110;
- Evidence Event Store self-test: 6;
- Minime semantic-stale parity: 36;
- Minime dynamic-noise parity: 16;
- Minime resonance-density parity: 17.

The six flywheel/event-store suites total 227 passing tests. The Corridor
regression specifically proves that an externally recorded Sandbox result
cannot be routed back into execution or source prep.

## Deployment And Alignment

Only the sanctioned wrapper was used:

`scripts/build_bridge.sh --ack "Run 10 telemetry-integration timing diagnostics and LLM quality-gate regressions; preserve all shared dirty-tree work; no live-control changes" --actor codex --restart`

- bridge PID: `73444 -> 61554`
- receipt: `env_receipt_1784269302752_188000`
- receipt status: passed
- actor: `codex`
- bridge binary SHA-256:
  `cacd05e85e538265db593ec8df82220c28031d4d92c787a091e7ab75bb19bd53`
- protocol: `1.0`
- protocol revision: `c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5`
- Astrid source identity:
  `63a7117b3c2eff578f28d1019b441a72980dd8e5`
- Minime source identity:
  `dda182f53fcf08d3b73cc97a3b2eb809a76de306`
- model source identity:
  `8173ed7d95df7ef0740c3f065693427f908e5ecd`

Ports `7878`, `7879`, and `8090` were listening. `/livez` and `/readyz`
returned HTTP 200 in 0.000841 s and 0.001162 s. Telemetry and fill were fresh
and readable; Minime was near its 68 percent shelf and contracting. Fresh
bridge logs showed post-restart autonomous exchanges without new error, warn,
panic, or fatal lines. The generated stack receipt passed protocol, revision,
manifest, binary, PID, process, log, telemetry, and model-readiness checks.

Restart alignment is current with no restart debt. Read-only recent-signal
generation after restart consumed fresh artifacts. It continued to report the
pre-existing physical camera/microphone capture surface as `needs_review`
while host-source freshness remained true; this run neither caused nor changed
that sensory state.

## Canonical Counters

Final cutoff:
`introspection_astrid_llm_1784267493.txt` (numeric timestamp 1784267493).

- canonical indexed: 2131
- canonical fully addressed: 1098
- canonical remaining: 1033
- canonical full read: 1414
- all indexed artifacts: 3586
- all pending artifacts: 2488
- tier mismatches: 0
- corrupt event lines: 0
- counter audit: consistent, every invariant true

The selected packet has no unprocessed filename. One newer canonical report
arrived during the run, making the net reduction 19 after 20 completions.

## Evidence Event Store V2

- active_store: `v2`
- valid hash chain: true
- global sequence: 35209
- head SHA-256:
  `4eb91b0bd5f37852bec34bc78f07fc95db0a5fc8faf0521c106c3b53c377e554`
- addressing events: 33524
- sandbox events: 1570
- Corridor V1 events: 3
- Corridor V2 events: 112
- authority violations: 0
- corrupt rows: 0

All frozen V1 hashes remain exact:

- addressing:
  `4a69dc092c1bcad8e157936f11f7798d67a883869bcfe56816fdf1be5ec78571`
- sandbox:
  `eac68fe839042c981756c2ec3b5c64f5a2633fdb75847a14fbd98c8f64ec4ebb`
- Corridor V1:
  `e190046e1b583d5b7b4a624ab50314fafbb6e0d751d9605f1ce9e85f148e01e4`
- Corridor V2:
  `e0ddb5e715d9a20cc709402fb1eda4712a1de001ea23730c70a349096468ccd5`

## Next Canonical Packet

1. `introspection_astrid_llm_1784267493.txt`
2. `introspection_minime_autonomous_agent_1783334949.txt`
3. `introspection_minime_main_excerpt_1783334385.txt`
4. `introspection_minime_esn_1783333936.txt`
5. `introspection_minime_sensory_bus_1783333553.txt`
6. `introspection_minime_regulator_1783333157.txt`
7. `introspection_astrid_llm_1783332795.txt`
8. `introspection_astrid_types_1783332484.txt`
9. `introspection_astrid_ws_1783332104.txt`
10. `introspection_astrid_autonomous_1783331197.txt`
11. `introspection_astrid_codec_1783330905.txt`
12. `introspection_proposal_12d_glimpse_1783330469.txt`
13. `introspection_astrid_llm_1783329079.txt`
14. `introspection_proposal_distance_contact_control_1783328552.txt`
15. `introspection_proposal_bidirectional_contact_1783328160.txt`
16. `introspection_proposal_phase_transitions_1783327645.txt`
17. `introspection_minime_autonomous_agent_1783327365.txt`
18. `introspection_minime_main_excerpt_1783327091.txt`
19. `introspection_minime_esn_1783326627.txt`
20. `introspection_minime_sensory_bus_1783326350.txt`

## Authority Waits

The selected packet contributes no Tier 4 wait and 18 exact Tier 5 waits.
Across active addressing work there are 23 Tier 4 and 843 Tier 5 items; the
Sandbox queue contains 17 Tier 4 and 834 Tier 5 trials. No pressure, fill, PI,
controller, cadence, rescue, sensory admission/retention, ESN rho/noise,
codec vector/gain/transport, provider route, Minime regulation, peer mutation,
phase behavior, or live-control authority was changed or granted.
