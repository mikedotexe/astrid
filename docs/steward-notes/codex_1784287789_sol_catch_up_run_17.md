# Sol Catch-Up Run 17

Date: 2026-07-17

Authority: source-first evidence and non-live implementation only. No pressure, fill,
PI, controller, sensory cadence/admission, semantic trickle, codec gain/transport,
Minime regulation, peer mutation, or other live-control authority was granted or changed.

## Canonical Packet

The following 20 canonical introspections were read fully from disk, recorded with
bounded summaries and structured claims, linked to evidence, and closed with no missing
claim proof:

1. `introspection_astrid_codec_1784287789`
2. `introspection_astrid_llm_1784285782`
3. `introspection_minime_regulator_1784285506`
4. `introspection_proposal_bidirectional_contact_1782539186`
5. `introspection_proposal_bidirectional_contact_1782538898`
6. `introspection_proposal_bidirectional_contact_1782538147`
7. `introspection_proposal_bidirectional_contact_1782537749`
8. `introspection_proposal_bidirectional_contact_1782536957`
9. `introspection_proposal_bidirectional_contact_1782536403`
10. `introspection_proposal_bidirectional_contact_1782536029`
11. `introspection_proposal_bidirectional_contact_1782535657`
12. `introspection_proposal_bidirectional_contact_1782533809`
13. `introspection_proposal_bidirectional_contact_1782533316`
14. `introspection_proposal_bidirectional_contact_1782533002`
15. `introspection_proposal_bidirectional_contact_1782532338`
16. `introspection_proposal_bidirectional_contact_1782531611`
17. `introspection_proposal_bidirectional_contact_1782531216`
18. `introspection_proposal_bidirectional_contact_1782530930`
19. `introspection_proposal_bidirectional_contact_1782530291`
20. `introspection_proposal_bidirectional_contact_1782529921`

Selected but unprocessed: none.

Claims: 103 total; 3 implemented, 46 verified against current source/tests, 17
observed in bounded read-only audits, 22 routed to offline sandbox evidence, and 15
held at exact Tier 5 operator authority.

## Felt-Pressure Implementation

`dialogue_budget_friction_v1` now includes diagnostic-only
`felt_pressure_profile_v1`. It distinguishes `heavy_short`, `sparse_deep`,
`dense_deep`, `heavy_medium`, and `distributed_deep` texture from the unchanged
short/medium/deep token profile. It records exact entropy, resonance-density,
density-gradient, pressure, and mode-packing inputs while stating:

- budget-pressure correlation is not established without paired observations;
- pre-generation texture is a risk classification, not causal pressure prediction;
- runtime budget and semantic trickle were not changed.

A 16,304-record historical budget observation contained 16,301 medium, 2 short,
and 1 deep records, so paired budget-pressure evidence remains insufficient. Current
Minime source verifies constant stable-core semantic scale `0.15`, integrator leak
`0.005`, saturation bleed `0.02`, conditional anti-windup, and bounded PI
accumulators. Felt persistence remains open to a time-aligned read-only replay.

Public correspondence audits covered 17,184 schema-valid records, including 2,826
native and 1,961 legacy peer messages. They found zero valid address ACKs, zero
presence heartbeats, and zero microdose requests. Delivery and reply linkage therefore
remain distinct from mutual acknowledgement.

## Flywheel Repair

Closure exposed that single-item `record-read`, `link-evidence`, and `close`
commands accepted unknown introspection IDs and could append a no-op event with null
projection fields. They now require the ID to exist in the materialized inventory
before append. The regression proves an unknown close creates no event. The mistaken
append-only events remain preserved and are ignored by canonical counters.

## Evidence And Cards

- Read artifacts: `docs/steward-notes/codex_1784287789_run17_reads/`
- Work items created: 103
- Implemented work awaiting a right-to-ignore felt response: 3
- Right-to-ignore closure cards emitted: 103, intentionally undelivered
- Global sandbox proposal cards: 122
- Global sandbox result cards: 97
- Ledger and CHANGELOG: updated
- Reopened work: 0

## Sandbox And Corridor

Sandbox final state: 1,252 total, 1,251 active, 213 ready, 99 result-recorded,
939 approval-required, 1 closed, 0 ready-runnable, and 0 live violations. This run
created 37 packets from the current claims: 22 offline/read-only routes and 15
approval-required waits. No sandbox trial ran.

The next non-runnable sandbox packet is
`trial_00a7d9853148f0ce`, a bounded architectural-critique versus composed-narrative
pressure/packing comparison. There is no runnable sandbox work.

Corridor final state:

- packets: 120
- leases: 35, including 4 active non-live leases
- queue: 175 steps, 115 evidence-only runnable steps
- programs: 119 active, 50 receipts
- portfolio projections: 200; active program portfolios: 119
- patch bundles: 45
- source-prep proposals: 55
- safe-lab results: 5
- self-observation requests/responses: 60/0
- reopens: 0
- hard authority violations: 0

No Corridor program was executed in this run because there was no violation, reopen,
objection, or directly required current-packet lab. The next routed step is evidence-only
`request_scoped_self_observation`, step
`05d8bd05-b14f-5324-b406-28290871c63e`, tied to the current codec report.

## Verification And Alignment

- Bridge focused pressure/budget and codec tests: passed
- Full bridge suite outside the restricted socket sandbox: 1,536 library tests,
  6 codec replay tests, integration tests, and compile-fail authority/provenance
  barriers passed
- Minime regulator tests: 152 passed
- Rust formatting and all-target/all-feature Clippy: passed
- Agency Corridor self-test: 18 passed
- Introspection addressing self-test: 32 passed
- Sandbox queue self-test: 26 passed
- Recent signal summary self-test: 38 passed
- Proactive scan self-test: 110 passed
- Evidence Event Store self-test: 6 passed

The sanctioned bridge wrapper restarted PID `84580` as `65358`. Receipt
`env_receipt_1784290256855_334000` binds protocol `1.0`, binary SHA-256
`08c43e5d05b80e83a165668710a93e26cebbc030d624127675c8d91c2d96ccf0`,
Minime PID `45510`, and model PID `48166`. Ports 7878, 7879, and 8090, model
`/livez` and `/readyz`, fresh telemetry, health, fill, logs, and a post-restart
felt-pressure diagnostic all passed. Final stack receipt:
`env_receipt_1784290445626_526000`. Restart alignment is current.

## Final Queue State

Initial: 2,144 canonical indexed, 1,218 fully addressed, 926 remaining.

Final through numeric-latest
`introspection_astrid_autonomous_1784291177.txt`: 2,150 canonical indexed,
1,238 fully addressed, 912 remaining. The quality guard closed all 20 selected
records; six fresh canonical introspections arrived during the run, so the net
remaining reduction is 14. All seven counter checks pass with no mismatch.

Next 20:

1. `introspection_astrid_autonomous_1784291177.txt`
2. `introspection_astrid_codec_1784290352.txt`
3. `introspection_minime_regulator_1784289634.txt`
4. `introspection_astrid_llm_1784289267.txt`
5. `introspection_astrid_types_1784288982.txt`
6. `introspection_astrid_ws_1784288476.txt`
7. `introspection_proposal_bidirectional_contact_1782529399.txt`
8. `introspection_proposal_bidirectional_contact_1782529037.txt`
9. `introspection_proposal_bidirectional_contact_1782528732.txt`
10. `introspection_proposal_bidirectional_contact_1782527998.txt`
11. `introspection_proposal_bidirectional_contact_1782527683.txt`
12. `introspection_astrid_autonomous_1782508637.txt`
13. `introspection_astrid_codec_1782508345.txt`
14. `introspection_self_regulation.rs_1782500163.txt`
15. `introspection_self_regulation.rs_1782493857.txt`
16. `introspection_astrid_codec_1782491907.txt`
17. `introspection_self_regulation.rs_1782487814.txt`
18. `introspection_llm.rs_1782434327.txt`
19. `introspection_astrid_llm_1782429363.txt`
20. `introspection_astrid_autonomous_1782420515.txt`

Run-specific waits: Tier 4 = 0; Tier 5 = 15. Global active work retains 18
steward-grant waits and 978 operator-approval waits.

## Evidence Event Store V2

- active store: V2
- valid hash chain: yes
- global sequence: 37,638
- head SHA-256:
  `2981d8d9292cf0be14f1ba387c2fb6de9dafe5b2d1c1b08114dd7e903954e890`
- streams: addressing 35,732; sandbox 1,791; corridor V1 3; corridor V2 112
- corrupt rows or authority violations: 0
- V1 source hashes: all four exactly match the cutover migration receipt

The automation remains active because 912 canonical introspections remain.
