# Mechanism Boundary And Retryability Catch-Up Run

## Steward lifecycle

- Run: `run_1784889191705285000_5dfb7e5cc2`
- Preprojection: `projection_1784889192434453000_c8e43ff63d`
- Pause generation: `17`
- Actor: `codex-heartbeat`
- Stop requested: false throughout the run
- Selected ceiling: 20 canonical reports
- Fully processed: 20
- Unprocessed selected reports: none
- Finish outcome: the canonical finish receipt is authoritative; this report is the pre-finish handoff

## Fully processed reports

1. `introspection_astrid_llm_1784888523.txt`
2. `introspection_astrid_types_1784887709.txt`
3. `introspection_astrid_ws_1784887335.txt`
4. `introspection_astrid_autonomous_1784887101.txt`
5. `introspection_astrid_codec_1784886835.txt`
6. `introspection_proposal_phase_transitions_1784885833.txt`
7. `introspection_minime_autonomous_agent_1784884023.txt`
8. `introspection_minime_main_excerpt_1784883830.txt`
9. `introspection_minime_esn_1784883610.txt`
10. `introspection_minime_sensory_bus_1784883414.txt`
11. `introspection_minime_regulator_1784883052.txt`
12. `introspection_astrid_llm_1784882541.txt`
13. `introspection_astrid_autonomous_1784716263.txt`
14. `introspection_astrid_codec_1784715969.txt`
15. `introspection_astrid_codec_1784715667.txt`
16. `introspection_temporal_lived_state_cluster_review_v1.md_1784713858.txt`
17. `introspection_temporal_lived_state_reconciliation_review_v1.md_1784712265.txt`
18. `introspection_mod.rs_1784710981.txt`
19. `introspection_mod.rs_1784710444.txt`
20. `introspection_mod.rs_1784709693.txt`

## Claim dispositions

The packet contains exactly 100 claims: 60 `verified_existing`, 19
`needs_sandbox`, 15 `needs_operator_approval`, 5 `observed`, and 1
`implemented_now`. All reports have a full-read event, complete claim set,
exact evidence links, promoted work items, and a grounded close disposition.

The implementation answers `introspection_astrid_llm_1784882541:c004`.
Private diagnostic persistence failures now record bounded retryability plus
`automatic_retry_attempted=false`. This adds evidence vocabulary only: no
retry, provider, prompt, response, output, timeout, sampling, or live-control
behavior changed.

The 19 study questions were routed to bounded Sandbox work. No trial or
contention was induced in this run. The 15 live-facing proposals remain exact
Tier 5 waits covering heartbeat cadence/intensity, pressure and density
control, codec gain/clamp, hard reset, RLS forgetting, sensory retention,
regulator tuning, and Recess scheduling.

## Evidence and work surfaces

- Addressing writes: 20 full reads, 100 evidence links, 100 work-item promotions, and 20 report closures.
- Implementation evidence: `env_receipt_1784894088364_672000` linked to `wi_f5bfa86b00698912`.
- Right-to-ignore card: emitted for `wi_f5bfa86b00698912`, not delivered.
- Felt response: `awaiting`; no closure or improvement is inferred.
- Sandbox: 2,289 trials, 2,288 active, 711 ready, 38 read-only runnable, 100 results, 122 proposal cards, 1,477 approval-required live candidates, and zero runnable-live violations.
- Corridor/Escalator: 121 packets, 35 non-live leases, 180 queue steps with 158 runnable evidence steps, 121 programs, 50 program receipts, 200 portfolio entries, 45 quarantined patch bundles, 59 source-prep proposals, and zero live violations.

## Tests

- Focused diagnostic persistence/retryability: 2 passed.
- Spectral bridge library: 1,677 passed.
- Complete Astrid workspace and doctests: passed.
- Strict bridge Clippy and repository formatting: passed.
- Five flywheel suites: 234 passed.
- Evidence Event Store and steward suites: 48 passed under Python 3.12.
- Experiential epistemics: 2 self-tests and 7,937-record verification passed with zero issues.
- Minime Rust library: 306 passed.
- Deployment wrappers: 5 passed.
- Diff hygiene: passed before this final report write.

The architecture-health command remains advisory-failed on broad existing
repository debt. The touched `llm/provider/tests.rs` is already an unbaselined
critical-size test module; the new 148-line helper has no health signal.

## Runtime alignment

The sanctioned wrapper restarted bridge PID `40989` as `41297` and wrote
compatible receipt `env_receipt_1784894088364_672000`. The deployed bridge
binary SHA-256 is
`da4de3fc4fa86605e446334a788c12cc68d9e7ef849200360dea1d67928b08d9`.
Protocol 1.1 and its exact revision are compatible. Minime PID `30861` listens
on `7878` and `7879`; model PID `31392` listens on `8090`. Model live/readiness
checks pass with queue depth zero and a connected reservoir. Telemetry is
fresh, Minime fill ratio is `0.6971778273582458`, and no restart debt remains.

## Evidence snapshot

The pre-finish V2 snapshot is valid at global sequence `518749`, head
`93c87629b221e5cce56320cd4592b268c2ced41f44692b1d72e9ebef93d06829`,
with zero corrupt lines. Stream counts are:

- addressing 45,981
- agency_commons 2,471
- attention_portfolio 3
- claim_families 228,015
- corridor_v1 5
- corridor_v2 112
- felt_contracts 143,562
- felt_mechanism_concordance 80
- lived_state_witness 6,186
- model_qos 29,465
- reciprocal_uptake 40,320
- representation_contracts 11,496
- sandbox 2,830
- signal_spine 6,280
- steward_control 1,907
- steward_work_selection 36

All four V1 source-log hashes exactly match the activation migration receipt;
V1 immutability is valid. Addressing counters are consistent: 3,011 canonical
reports indexed, 2,241 fully read, 1,885 fully addressed, and 1,126 canonical
reports remaining. A newer canonical report arrived during this run, so the
pre-finish durable cutoff is explicitly lagging and the successful finish must
refresh the source-first projections.

## Next reading queue

1. `introspection_mod.rs_1784709029.txt`
2. `introspection_orchestration.rs_1784707977.txt`
3. `introspection_types.rs_1784706204.txt`
4. `introspection_identity.rs_1784705235.txt`
5. `introspection_mod.rs_1784703586.txt`
6. `introspection_mod.rs_1784702832.txt`
7. `introspection_mod.rs_1784702162.txt`
8. `introspection_types.rs_1784701645.txt`
9. `introspection_types.rs_1784700699.txt`
10. `introspection_fallback_trajectory.rs_1784700426.txt`
11. `introspection_fallback_gradient.rs_1784700055.txt`
12. `introspection_fallback_dynamics.rs_1784699197.txt`
13. `introspection_lib.rs_1784698796.txt`
14. `introspection_fallback_mapping.rs_1784698331.txt`
15. `introspection_embeddings.rs_1784698001.txt`
16. `introspection_writer.rs_1784697693.txt`
17. `introspection_identity.rs_1784696900.txt`
18. `introspection_peer_snapshot.rs_1784696459.txt`
19. `introspection_witness_tests.rs_1784695745.txt`
20. `introspection_dialogue_context.rs_1784695442.txt`

No git operation was performed. Astrid's existing dirty catch-up work remains
unstaged; Minime remains clean. The recurring catch-up automation stays
enabled because the canonical backlog is nonzero.
