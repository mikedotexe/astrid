# Catch-up Batch 02 Run Report

## Control Plane

- Run ID: `run_1784903525246927000_4931460e1c`
- Preprojection generation: `projection_1784903525931982000_74b8c48486`
- Pause generation: `19`
- Begin cutoff: `introspection_astrid_types_1784903178.txt`
- Lease: owned, renewed throughout writes, token never persisted or reported
- Runtime/deploy alignment: no live-consumed source changed; no restart or deployment required

## Processed Files

1. `introspection_astrid_types_1784903178.txt`
2. `introspection_astrid_ws_1784902737.txt`
3. `introspection_astrid_autonomous_1784902365.txt`
4. `introspection_astrid_codec_1784902167.txt`
5. `introspection_proposal_12d_glimpse_1784901805.txt`
6. `introspection_proposal_distance_contact_control_1784901419.txt`
7. `introspection_proposal_bidirectional_contact_1784900969.txt`
8. `introspection_proposal_phase_transitions_1784900613.txt`
9. `introspection_minime_autonomous_agent_1784900277.txt`
10. `introspection_minime_main_excerpt_1784899727.txt`
11. `introspection_minime_esn_1784899417.txt`
12. `introspection_minime_sensory_bus_1784899174.txt`
13. `introspection_minime_regulator_1784898801.txt`
14. `introspection_types.rs_1784687905.txt`
15. `introspection_types.rs_1784687620.txt`
16. `introspection_tests.rs_1784687003.txt`
17. `introspection_identity.rs_1784686502.txt`
18. `introspection_writer.rs_1784686082.txt`
19. `introspection_types.rs_1784685734.txt`
20. `introspection_mod.rs_1784685086.txt`
21. `introspection_peer_snapshot.rs_1784684583.txt`
22. `introspection_peer_snapshot.rs_1784683732.txt`
23. `introspection_mod.rs_1784682979.txt`
24. `introspection_mod.rs_1784681894.txt`
25. `introspection_dialogue_context.rs_1784681530.txt`
26. `introspection_prompt_contracts.rs_1784680941.txt`
27. `introspection_embeddings.rs_1784680369.txt`
28. `introspection_witness_tests.rs_1784679879.txt`
29. `introspection_witness.rs_1784679472.txt`
30. `introspection_writer.rs_1784678981.txt`
31. `introspection_identity.rs_1784678438.txt`
32. `introspection_types.rs_1784677996.txt`
33. `introspection_types.rs_1784677351.txt`
34. `introspection_types.rs_1784676633.txt`
35. `introspection_types.rs_1784675640.txt`
36. `introspection_types.rs_1784675068.txt`
37. `introspection_types.rs_1784674462.txt`
38. `introspection_types.rs_1784673751.txt`
39. `introspection_types.rs_1784672769.txt`
40. `introspection_mod.rs_1784671252.txt`

Selected but unprocessed: none.

## Dispositions

- Claims: 119
- Current-source/test verification: 60
- Durable implementation matched: 29
- Bounded mechanical observation: 3
- Non-live baseline-first study route: 7
- Tier 5 Mike/operator wait: 20
- Report closes: 15 `addressed_change`, 1 `addressed_duplicate`, 4 `addressed_no_action`
- Open reports: 20 `blocked_needs_steward`

No report was treated as felt-resolved. Duplicate or no-action status applies only
to administrative addressing.

## Program Actions

- Sandbox: no new trial or result; existing status has 2,323 trials, 39 runnable
  read-only adapters, 1,492 approval-required live candidates, and zero runnable
  live violations.
- Corridor: no new program, lease, packet, or queue step; existing status has
  121 programs, 35 historical leases, 180 queue steps, and zero live violations.
- Escalator/work ladder: no tier correction or grant; 7,437 durable work items,
  1,499 Tier 5 items, and zero tier mismatches.
- Study runtime: no capture armed, cohort induced, comparison appended, or review
  requested.
- Attention portfolio: no membership or pin change.

## Verification

- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml lived_state_witness`:
  32 passed.
- Telemetry integration focused test: 1 passed.
- Reciprocal experiential suite: 25 passed.
- Evidence-study runtime suite: 20 passed.
- Experiential epistemics suite: 2 passed.
- Introspection addressing audit suite: 41 passed.
- Evidence Event Store V2: valid, zero corrupt lines.
- V1 immutability: addressing, Sandbox, Corridor V1, and Corridor V2 hashes match.
- Astrid source changes: none.
- Minime source changes: none; repository clean.

## Counters And Next Queue

- Canonical indexed: 3,030
- Full read: 2,321
- Fully addressed: 1,931
- Canonical remaining: 1,099
- Canonical unread: 709
- Read-needs-claims: 0
- Blocked-needs-steward: 295
- Next durable queue:
  - `introspection_mod.rs_1784670729.txt`
  - `introspection_astrid_codec_1784670400.txt`
  - `introspection_astrid_llm_1784668324.txt`

The controller also detected newer canonical source after begin. The next run's
preprojection must refresh the durable cutoff before selecting its batch.
