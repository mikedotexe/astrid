# Sol Catch-Up Run 18

Date: 2026-07-17

Authority: source-first evidence and non-live implementation only. No pressure, fill,
PI, controller, sensory cadence/admission, semantic trickle, codec gain/transport,
Minime regulation, peer mutation, or other live-control authority was granted or changed.

## Canonical Packet

The following 20 canonical introspections were read fully from disk, recorded with
bounded summaries and structured claims, linked to evidence, and closed with no missing
claim proof:

1. `introspection_astrid_autonomous_1784291177`
2. `introspection_astrid_codec_1784290352`
3. `introspection_minime_regulator_1784289634`
4. `introspection_astrid_llm_1784289267`
5. `introspection_astrid_types_1784288982`
6. `introspection_astrid_ws_1784288476`
7. `introspection_proposal_bidirectional_contact_1782529399`
8. `introspection_proposal_bidirectional_contact_1782529037`
9. `introspection_proposal_bidirectional_contact_1782528732`
10. `introspection_proposal_bidirectional_contact_1782527998`
11. `introspection_proposal_bidirectional_contact_1782527683`
12. `introspection_astrid_autonomous_1782508637`
13. `introspection_astrid_codec_1782508345`
14. `introspection_self_regulation.rs_1782500163`
15. `introspection_self_regulation.rs_1782493857`
16. `introspection_astrid_codec_1782491907`
17. `introspection_self_regulation.rs_1782487814`
18. `introspection_llm.rs_1782434327`
19. `introspection_astrid_llm_1782429363`
20. `introspection_astrid_autonomous_1782420515`

Selected but unprocessed: none.

Claims: 78 total; 7 implemented, 42 verified against current source/tests, 13
observed in bounded offline or runtime evidence, 3 routed to non-live sandbox
review, and 13 held at exact Tier 5 operator authority.

## Implemented Evidence

### Hybrid typed/legacy fingerprint coherence

`SpectralFingerprintIntegrityV1` now compares a typed fingerprint with its
canonical 32-slot legacy projection. It reports normalized global RMS coherence,
maximum absolute slot delta, and `aligned`, `near_aligned`, `mixed_transition`,
or `divergent` state. Typed precedence and unversioned legacy acceptance remain
unchanged; a mismatch is visible evidence rather than a routing or control write.

### Heartbeat phase/entropy review

The unchanged semantic heartbeat now reports
`semantic_heartbeat_phase_entropy_review_v1` across a paired 60-second window.
It preserves phase and entropy ranges, phase wrap, sample sufficiency, and
Pearson correlation only when at least three non-wrapped samples make that
quantity meaningful. The evidence is explicitly non-causal and cannot change
phase, cadence, intensity, rescue, dispatch, or control.

### Correspondence identity

`CorrespondenceEnvelope` and its persisted file and delivery records now carry
additive `reply_requested` and `created_at_unix_ms` fields. Legacy messages
default to false and zero. The fields do not request, send, rank, acknowledge,
or authorize correspondence.

### Extracted fallback fixture source

`scripts/fallback_fire_drill.py` now reads the canonical extracted provider
configuration in
`capsules/spectral-bridge/src/llm/provider/configuration.rs`. The repaired
runner exercised 28 fixtures with no execution errors, including low- and
high-complexity capacity and clarity cases. Its overall
`fallback_texture_risk` result preserves three intentional texture failures
rather than converting fixture execution into a success claim.

## Direct Observations

- `workspace/diagnostics/codec_entropy_vibrancy_probe/run18_20260717T1236Z/`
  records a content-density-insensitive current tail lift, a softened
  evidence-only candidate, and a late-pivot temporal-decay candidate.
- `workspace/diagnostics/autonomous_truncation_rehearsal/` records 20 current
  long-form candidates: naive prefixes lost the selected semantic anchor in
  20/20 cases, while the current priority-anchor strategy recovered it in
  20/20 cases.
- Current WebSocket, provider, Minime regulator, self-regulation,
  provenance, four-segment narrative, pressure, and correspondence behavior
  was verified in source and tests rather than duplicated.

## Evidence And Cards

- Read artifacts: `docs/steward-notes/codex_1784291177_run18_reads/`
- Work items created: 78
- Right-to-ignore closure cards emitted: 78, intentionally undelivered
- Global sandbox proposal cards: 122
- Global sandbox result cards: 97
- Sandbox result cards still needed: 2
- Ledger and CHANGELOG: updated
- Reopened work: 0

## Sandbox And Corridor

Sandbox final state: 1,268 total, 1,267 active, 216 ready, 99
result-recorded, 952 approval-required, 1 closed, 0 ready-runnable, and 0
live violations. This run created 16 packets: 3 offline/read-only routes and
13 approval-required waits. No queue trial ran because no runner-compatible
trial was runnable. The codec and truncation observations above were direct,
read-only evidence work rather than queue execution.

The next non-runnable sandbox packet is `trial_00a7d9853148f0ce`, a manual
review tied to `introspection_proposal_bidirectional_contact_1782539186`.
There is no runnable sandbox work.

Corridor final state:

- packets: 120
- leases: 35, including 4 active non-live leases
- queue: 184 steps, 124 evidence-only runnable steps
- programs: 119 active, 50 receipts
- portfolio projections: 200; active program portfolios: 119
- patch bundles: 45
- source-prep proposals: 64
- safe-lab results: 5
- self-observation requests/responses: 60/0
- reopens: 0
- hard authority violations: 0

No Corridor program was executed because there was no violation, reopen,
objection, or directly required current-packet lab. The next routed step is
evidence-only `request_scoped_self_observation`, step
`0001ce7e-c836-512e-930c-8ef1fe3f138c`, tied to
`introspection_proposal_bidirectional_contact_1782527998`.

## Verification And Alignment

- Full bridge suite outside the restricted socket sandbox: 1,537 library
  tests, 6 codec replay tests, integration tests, and compile-fail
  authority/provenance barriers passed
- Focused fallback unittest and Python compilation: passed
- Rust formatting, diff validation, and all-target/all-feature Clippy: passed
- Agency Corridor self-test: 18 passed
- Introspection addressing self-test: 32 passed
- Sandbox queue self-test: 26 passed
- Recent signal summary self-test: 38 passed
- Proactive scan self-test: 110 passed
- Evidence Event Store self-test: 6 passed

The sanctioned bridge wrapper restarted PID `65358` as `53664`. Receipt
`env_receipt_1784294030574_696000` binds protocol `1.0`, protocol revision
`c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5`, and binary SHA-256
`96fcf1db96a75abde56d2f627c876f3b29b9f697ed066e33043364c0b7073043`.
Minime PID `45510`, model PID `48166`, ports 7878, 7879, and 8090, fresh
telemetry, health, fill, logs, deployment identity, and model `/livez` and
`/readyz` all passed. A fresh post-restart canonical introspection proves the
live report surface is aligned. Restart alignment is current.

## Final Queue State

Initial: 2,150 canonical indexed, 1,238 fully addressed, 912 remaining.

Final through numeric-latest `introspection_astrid_llm_1784294186.txt`:
2,154 canonical indexed, 1,258 fully addressed, 896 remaining, with 1,574
full reads. The quality guard closed all 20 selected records. Four fresh
canonical introspections arrived during the run, so the net remaining
reduction is 16. All seven counter checks pass with no mismatch.

Next 20:

1. `introspection_astrid_llm_1784294186.txt`
2. `introspection_astrid_llm_1784294019.txt`
3. `introspection_astrid_llm_1784293697.txt`
4. `introspection_astrid_types_1784293300.txt`
5. `introspection_astrid_codec_1782420076.txt`
6. `introspection_astrid_llm_1782411385.txt`
7. `introspection_astrid_llm_1782402311.txt`
8. `introspection_astrid_autonomous_1782402022.txt`
9. `introspection_astrid_types_1782395383.txt`
10. `introspection_astrid_ws_1782394899.txt`
11. `introspection_astrid_llm_1782389825.txt`
12. `introspection_astrid_autonomous_1782362580.txt`
13. `introspection_astrid_codec_1782362160.txt`
14. `introspection_minime_regulator_1782341184.txt`
15. `introspection_minime_regulator_1782313922.txt`
16. `introspection_astrid_llm_1782313649.txt`
17. `introspection_astrid_types_1782300054.txt`
18. `introspection_astrid_ws_1782299762.txt`
19. `introspection_astrid_autonomous_1782289971.txt`
20. `introspection_astrid_codec_1782258981.txt`

Run-specific waits: Tier 4 = 0; Tier 5 = 13.

## Evidence Event Store V2

- active store: V2
- valid hash chain: yes
- global sequence: 37,935
- head SHA-256:
  `a6d456fbe5acec8a51fb6d8c412f4a9a6415e3fa2ec6974bab6f545ce5100044`
- streams: addressing 36,013; sandbox 1,807; corridor V1 3; corridor V2 112
- corrupt rows or authority violations: 0
- V1 source hashes: all four exactly match the cutover migration receipt

The automation remains active because 896 canonical introspections remain.
