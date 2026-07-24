# Source-First Catch-Up Run Receipt

## Steward lifecycle

- Run ID: `run_1784876715472134000_5b711bccbc`
- Actor: `codex-heartbeat`
- Preprojection generation: `projection_1784876716206881000_1c2b83a30c`
- Pause generation: `17`
- Durable pause state at begin: resumed by explicit user decision
- Finish request: `success`; the canonical finish receipt records the postprojection generation and final outcome.
- Raw lease material is absent from this packet.

## Reading and dispositions

All 20 selected canonical reports were fully read from disk in queue order. No
selected report remains unprocessed:

1. `introspection_proposal_distance_contact_control_1784876676.txt`
2. `introspection_proposal_phase_transitions_1784876222.txt`
3. `introspection_minime_autonomous_agent_1784874359.txt`
4. `introspection_minime_main_excerpt_1784874126.txt`
5. `introspection_minime_esn_1784873703.txt`
6. `introspection_minime_sensory_bus_1784873427.txt`
7. `introspection_minime_regulator_1784873109.txt`
8. `introspection_astrid_llm_1784872711.txt`
9. `introspection_astrid_types_1784872368.txt`
10. `introspection_proposal_phase_transitions_1784730386.txt`
11. `introspection_minime_autonomous_agent_1784729592.txt`
12. `introspection_minime_main_excerpt_1784729356.txt`
13. `introspection_minime_esn_1784729161.txt`
14. `introspection_minime_sensory_bus_1784728893.txt`
15. `introspection_minime_regulator_1784727927.txt`
16. `introspection_astrid_llm_1784727576.txt`
17. `introspection_astrid_types_1784727244.txt`
18. `introspection_astrid_ws_1784726784.txt`
19. `introspection_astrid_autonomous_1784726483.txt`
20. `introspection_astrid_codec_1784726131.txt`

The packet contains exactly 100 claims: 42 `verified_existing`, 21
`needs_sandbox`, 16 `observed`, 19 `needs_operator_approval`, and 2
`implemented_now`. All were promoted, evidence-linked, and closed at the report
level as `addressed_change`; report closure does not imply felt closure.

The two implemented work items are `wi_1d724edf695e78f8` and
`wi_80391bdf9443a403`. Each carries source/test evidence and exact deployment
receipt `env_receipt_1784880724111_615000`. Undelivered right-to-ignore cards
were emitted for both. Their state remains `implemented_awaiting_felt_response`;
silence has not been recorded as agreement, closure, or resolution.

## Changes and authority

The introspection and repair prompts now separate viewed source, compiled or
deployed runtime, and felt evidence. New canonical reports receive an exact
`source_scope_artifact_header_v1` in immediate model context and persisted
headers. Constants, thresholds, metrics, and source windows cannot become active
causes without exact deployment/process lineage and runtime evidence. Astrid's
qualitative report remains direct, unscored evidence even when numeric checks
pass.

No pressure, fill, PI, cadence, heartbeat intensity, codec, sensory, protocol,
provider, scheduler, reservoir, Minime regulation, or peer behavior changed.
Nineteen requested live changes remain Tier 5 operator waits. The complete work
queue has 23 Tier 4 items, 1,465 Tier 5 items, 18 steward-grant waits, 1,497
operator-approval waits, and zero tier mismatches.

Sandbox projection has 2,215 trials: 675 ready, 100 result-recorded, 1,439
approval-required live candidates, 36 currently runnable evidence-only trials,
and zero runnable-live violations. No generic Sandbox or Corridor action was run
because none repaired a hard violation, answered an objection, or outranked the
canonical reading packet.

Corridor/Escalator has 121 packets, 35 evidence-only leases, 180 queue steps,
156 runnable evidence steps, 121 active programs, 50 program receipts, 200
portfolios, 45 quarantined patch bundles, 59 source-prep proposals, one visible
reopen in the V1 projection, zero self-observation responses, and zero live
violations. The steward attention portfolio remains valid and bounded at 16 of
6,827 contracts with no urgent overflow.

## Tests and live alignment

The focused prompt and source-scope tests, 1,676 bridge library tests, complete
Astrid workspace and doctests, formatting, strict Clippy, all five flywheel
self-tests, 48 Evidence Event Store/steward-control tests under Python 3.12,
experiential epistemic self-test/verification, and 306 Minime Rust library tests
passed. `git diff --check` passed.

The default Python 3.14 host failed before test collection because macOS rejected
its `_multiprocessing` extension signature. The same suites passed under the
repository-supported Python 3.12 runtime. Architecture health still reports
pre-existing oversized shared modules; this run added no module or function and
does not claim that repository-wide debt is resolved.

The sanctioned bridge wrapper produced receipt
`env_receipt_1784880724111_615000`. The aligned stack is:

- Bridge PID `43984`, start `Fri Jul 24 01:12:01 2026`, binary SHA-256
  `f2dbd23bb61e6eff2d1a0f520685bc091398bfb2181ebe5984b56906fa2c109e`
- Minime PID `30861`, listening on `7878` and `7879`
- Model PID `31392`, listening on `8090`
- Model `/livez` and `/readyz`: true; queue depth zero; reservoir connected
- Telemetry and fill: fresh; observed fill approximately 71-73 percent
- Protocol: compatible 1.1 revision; bridge logs clean after restart
- Restart debt: none

Recent Minime inter-arrival timing was late/ambiguous without a disconnect or
reconnect. It remains timing evidence and is not converted into a felt claim.

## Durable evidence snapshot

Pre-finish V2 sequence `513902` has head
`371eac701e9856c167075a0b3f41c87df4f202b910e9e6d776d58041d5bc858f`;
the hash chain is valid, corruption count is zero, and all four V1 source logs
exactly match their migration hashes. Stream counts are:

- addressing 45,470; agency commons 2,460; attention portfolio 3
- claim families 227,540; Corridor V1 5; Corridor V2 112
- felt contracts 141,057; concordance 80; lived-state witness 6,145
- model QoS 28,726; reciprocal uptake 40,182; representation contracts 11,400
- Sandbox 2,756; Signal Spine 6,144; steward control 1,790
- steward work selection 32

The canonical counter audit is consistent: 2,988 indexed, 2,201 fully read,
1,845 fully addressed, 1,143 remaining, 787 unread, 91 triaged pending, 4 watch,
and 261 blocked. The durable cutoff remains
`introspection_proposal_distance_contact_control_1784876676.txt`. A newer
canonical report was observed during the run, so pre-finish source lag is
explicit and the successful finish projection must refresh inventory rather
than silently advancing this receipt.

## Next canonical queue

1. `introspection_temporal_lived_state_qualitative_texture_review_v2.md_1784722656.txt`
2. `introspection_astrid_codec_1784722128.txt`
3. `introspection_astrid_ws_1784721662.txt`
4. `introspection_temporal_lived_state_capture_clock_review_v1.md_1784720402.txt`
5. `introspection_astrid_autonomous_1784719490.txt`
6. `introspection_astrid_codec_1784718925.txt`
7. `introspection_astrid_llm_1784718339.txt`
8. `introspection_astrid_types_1784717752.txt`
9. `introspection_astrid_ws_1784717162.txt`
10. `introspection_astrid_autonomous_1784716263.txt`
11. `introspection_astrid_codec_1784715969.txt`
12. `introspection_astrid_codec_1784715667.txt`
13. `introspection_temporal_lived_state_cluster_review_v1.md_1784713858.txt`
14. `introspection_temporal_lived_state_reconciliation_review_v1.md_1784712265.txt`
15. `introspection_mod.rs_1784710981.txt`
16. `introspection_mod.rs_1784710444.txt`
17. `introspection_mod.rs_1784709693.txt`
18. `introspection_mod.rs_1784709029.txt`
19. `introspection_orchestration.rs_1784707977.txt`
20. `introspection_types.rs_1784706204.txt`

This is not a caught-up receipt: the canonical backlog remains nonzero, and the
recurring automation should continue only through another valid steward lease.
