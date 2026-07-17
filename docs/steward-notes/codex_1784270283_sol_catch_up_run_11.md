# Sol Source-First Catch-Up Run 11

- run_id: `codex_1784270283_sol_catch_up_run_11`
- authority: evidence, diagnostics, and runtime-adjacent reporting only
- selected_packet_count: 20
- fully_processed_count: 20
- selected_packet_unprocessed: none
- canonical_start_remaining: 1033
- canonical_final_remaining: 1013
- gross_quality_guard_reduction: 20
- net_quality_guard_reduction: 20
- new_canonical_arrivals_during_run: 0
- restart_alignment: current

## Full-Read Packet

The packet was read fully from disk in canonical queue order: 500 lines and
61,488 bytes in total.

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

Bounded summaries, all claim records, the record-read manifest, and evidence
links are in `docs/steward-notes/codex_1784270283_run11_reads/`.

## Claim Dispositions

- implemented_now: 3
- verified_existing: 49
- needs_sandbox: 12
- tier_5_wait: 18
- tier_4_wait in selected packet: 0
- unsupported no-action dispositions: 0

All 20 reports have durable full-read events, bounded summaries, claim
dispositions, evidence links, `addressed_change` closures, and right-to-ignore
closure cards. The run wrote 109 evidence links, promoted 82 work items,
closed 20 reports, and emitted 82 closure cards.

One append-only correction changed work item `wi_e8b13d60d3920d23` from an
incorrect Tier 5 classification to Tier 1 `verified_existing`. The historical
summary said that behavior "should change", but the current claim explicitly
verified that the implementation is already read-only. The correction granted
no authority and preserved the original event.

## Implemented Evidence

Model-artifact cleanup now records placement-aware evidence for boundary,
contextual, and quoted marker occurrences while retaining the existing output
stripping behavior. `ModelArtifactSemanticIntegrityCheckV1` distinguishes:

- `review_output_erased`
- `review_contextual_marker_removal`
- `review_high_removal_fraction`
- `structural_cleanup_low_risk`

The diagnostic explicitly states that intent preservation is not established.
Tests cover quoted/contextual marker text, low-risk boundary cleanup, and exact
512/513 and 1024/1025 budget boundaries.

The addressing classifier now gives an explicit current `verified_existing`
claim disposition precedence over proposal language inherited from an older
claim summary. A focused self-test reproduces and prevents the false Tier 5
escalation.

Source verification also established that current implementations already
provide dynamic pressure trend windows, telemetry-gap classification, rich
viscosity/fidelity/drift evidence, read-only Recess density mitigation,
divergence buffers, stale-source windows, bounded handover and hysteresis,
entropy persistence, smoothstep surge tapering, dynamic noise/coherence,
dynamic fallback texture, and first-class 12D/correspondence/phase evidence.
These are verifications, not permission to alter live behavior.

## Sandbox

Run 11 routed 30 claims: 12 for sandbox observation and 18 exact Tier 5 waits.
Eleven sandbox requests remain classified manual reviews with no executable
adapter. One directly relevant read-only trial ran:

- trial: `trial_f928c6d82147b07e`
- adapter: `shadow_influence_replay_v1`
- classification: `replay_supports_bounded_shadow_gain`
- requested multiplier: 1.0
- sample count: 1
- base classification: `lattice_transition_like`
- average norm delta: 0.0
- average dispersal delta: 0.0
- fragmentation flags: none
- live mutation: none
- result:
  `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/results/1784271787_trial_f928c6d82147b07e_shadow_influence_replay_v1.json`

The result supports only the bounded replay case. A broader high-fill
comparison remains open because one sample cannot establish a general effect.
One result card was emitted; proposal cards were unchanged.

Final Sandbox state:

- total: 1102
- active: 1101
- ready: 158
- result_recorded: 89
- approval_required: 854
- closed: 1
- ready_runnable: 0
- proposal_cards: 103
- result_cards: 87
- live violations: 0
- Tier 4: 17
- Tier 5: 852

## Corridor And Escalator

Corridor/program work was regenerated and audited for routing visibility.
Generic ready bundles were not run because there was no hard violation,
objection, reopened friction, or packet-linked safe lab requiring them; doing
so would have displaced canonical reading. All authority projections remain
false.

Final routing state:

- packets: 120 total; 55 canary proposals, 5 safe results, 60 self-observation requests
- leases: 35 total, 4 active, 31 imported evidence-only, 0 revoked
- queue steps: 189
- runnable non-live steps: 129
- programs: 118
- stored portfolios: 200
- current-program portfolios: 118
- patch bundles: 45
- source-prep proposals: 69
- reopened work items: 0
- self-observation responses: 0
- hard violations: 0

The next runnable Corridor step is bounded
`request_scoped_self_observation` step
`0b8dc084-5f61-5af2-815f-d743989f597d` for
`introspection_astrid_ws_1783332104`. It is evidence-only, grants no approval,
and cannot make live work executable. There is no runnable Sandbox trial.

## Tests

Passed:

- model-artifact cleanup and budget-boundary focused bridge tests;
- bridge library suite: 1525 tests;
- bridge codec replay and integration suites, including authority/provenance
  typestate, chimera, and mock-WebSocket coverage;
- strict bridge Clippy across all targets and features;
- bridge formatting;
- shared protocol wire-contract suite: 6 tests;
- introspection addressing self-test: 30;
- Agency Corridor self-test: 18;
- sandbox queue self-test: 26;
- recent signal self-test: 38;
- proactive scan self-test: 110;
- Python compilation for the changed addressing script;
- Minime semantic-stale parity: 36;
- Minime surge-target parity: 4;
- Minime shadow-influence parity: 10;
- Minime dynamic-noise parity: 16;
- Minime warm-start review: 2;
- Minime Python autonomy/Recess suite: 268.

The first sandboxed full bridge and Minime Python runs encountered only
filesystem permission failures. Their unrestricted reruns passed completely.
No Minime source changed in this run.

## Deployment And Alignment

Only the sanctioned wrapper was used:

`scripts/build_bridge.sh --ack "Run 11 Astrid-grounded model artifact semantic-integrity diagnostics and addressing classifier repair; no live-control, protocol, pressure, fill, PI, cadence, codec gain, or dispatch behavior change" --actor codex --restart`

- bridge PID: `61554 -> 32293`
- receipt: `env_receipt_1784272510395_290000`
- receipt status: passed and compatible
- actor: `codex`
- bridge binary SHA-256:
  `8ee1b86be0deb400fda13c58343d8af386091bec14fe77e22d099240d2f49a25`
- protocol: `1.0`
- protocol revision:
  `c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5`
- Astrid source identity:
  `63a7117b3c2eff578f28d1019b441a72980dd8e5`
- Minime source identity:
  `dda182f53fcf08d3b73cc97a3b2eb809a76de306`
- model source identity:
  `8173ed7d95df7ef0740c3f065693427f908e5ecd`

The receipt is witness-only and every authority marker is false. Stack PIDs
were bridge 32293, Minime 45510, and model 48166, with process start identities
captured. Ports 7878, 7879, and 8090 were listening. `/livez` reported live and
`/readyz` remained ready while generation was active, with reservoir
connectivity and an empty queue. Bridge telemetry, Minime health, and host
telemetry were fresh; fill was 70.896 percent. Fresh logs showed both
WebSocket lanes connected and restored continuity. A telemetry broken pipe
after 70 seconds recovered in one second, followed by fresh telemetry and
autonomous output.

Restart alignment is current with no restart debt.

## Canonical Counters

Final cutoff:
`introspection_astrid_llm_1784267493.txt` (numeric timestamp 1784267493).

- canonical indexed: 2131
- canonical fully addressed: 1118
- canonical remaining: 1013
- canonical full read: 1434
- addressed_change: 1013
- duplicate: 43
- no_action: 62
- blocked: 261
- triaged_pending: 51
- watch: 4
- unread: 697
- tier mismatches: 0
- corrupt event lines: 0
- counter audit: consistent, all seven invariants true

The selected packet has no unprocessed filename, and no newer canonical report
arrived during the run.

## Evidence Event Store V2

- active_store: `v2`
- valid hash chain: true
- global sequence: 35565
- head SHA-256:
  `4f5d6b42e5f1eed17fb4b6eacf7701a063e61d388b2d0e8a8da9986131c56943`
- addressing events: 33848
- sandbox events: 1602
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

1. `introspection_minime_regulator_1783325745.txt`
2. `introspection_astrid_codec_1783144888.txt`
3. `introspection_astrid_llm_1783135338.txt`
4. `introspection_autonomous.rs_1783095476.txt`
5. `introspection_autonomous.rs_1783093285.txt`
6. `introspection_regulator.rs_1783085935.txt`
7. `introspection_astrid_llm_1783054630.txt`
8. `introspection_astrid_llm_1783006290.txt`
9. `introspection_minime_sensory_bus_1782999970.txt`
10. `introspection_minime_regulator_1782999536.txt`
11. `introspection_astrid_llm_1782999056.txt`
12. `introspection_minime_regulator_1782988151.txt`
13. `introspection_astrid_llm_1782984081.txt`
14. `introspection_astrid_types_1782983557.txt`
15. `introspection_astrid_ws_1782983284.txt`
16. `introspection_autonomous.rs_1782980142.txt`
17. `introspection_astrid_llm_1782971249.txt`
18. `introspection_astrid_autonomous_1782968352.txt`
19. `introspection_astrid_autonomous_1782968180.txt`
20. `introspection_astrid_codec_1782967589.txt`

## Authority Waits

The selected packet contributes no Tier 4 wait and 18 exact Tier 5 waits.
Across active addressing work there are 23 Tier 4 and 861 Tier 5 items, with
911 total grant waits. The Sandbox queue contains 17 Tier 4 and 852 Tier 5
trials. No pressure, fill, PI, controller, cadence, rescue, sensory
admission/retention, ESN rho/noise, codec vector/gain/transport, provider
route, Minime regulation, peer mutation, phase behavior, or live-control
authority was changed or granted.
