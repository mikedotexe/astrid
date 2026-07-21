# Source-First Catch-Up Run 07

Date: 2026-07-20

Steward run: `run_1784590559370932000_ab8b6a5cc5`

Preprojection generation: `projection_1784590559823538000_dd2b12c5c6`

## Canonical Packet

All 20 selected reports were read fully from disk in canonical queue order:

1. `introspection_astrid_autonomous_1784589075.txt`
2. `introspection_astrid_autonomous_1784588967.txt`
3. `introspection_astrid_ws_1784588143.txt`
4. `introspection_astrid_autonomous_1784587759.txt`
5. `introspection_minime_regulator_1784586129.txt`
6. `introspection_astrid_codec_1784585858.txt`
7. `introspection_proposal_12d_glimpse_1784585665.txt`
8. `introspection_proposal_distance_contact_control_1784585353.txt`
9. `introspection_proposal_bidirectional_contact_1784584935.txt`
10. `introspection_proposal_phase_transitions_1784584556.txt`
11. `introspection_minime_autonomous_agent_1784584264.txt`
12. `introspection_minime_main_excerpt_1784583673.txt`
13. `introspection_minime_esn_1784583270.txt`
14. `introspection_minime_sensory_bus_1784582936.txt`
15. `introspection_minime_regulator_1784582153.txt`
16. `introspection_astrid_llm_1784581897.txt`
17. `introspection_astrid_types_1784581688.txt`
18. `introspection_astrid_ws_1784581317.txt`
19. `introspection_astrid_autonomous_1784580708.txt`
20. `introspection_astrid_codec_1784580201.txt`

The full-read identity, bounded summaries, and claim files are under
`docs/steward-notes/codex_1784589075_run07_reads/`.

## Claim Dispositions

- 83 claims extracted with no unsupported no-action dispositions.
- 42 `verified_existing` from current source or focused tests.
- 5 `observed` with explicit causal limits.
- 2 `implemented`: first-valid connection spectral entropy and exact 11D/13D malformed-glimpse regressions.
- 16 `sandbox_routed` as bounded replay, comparison, or temporal observation work.
- 18 `authority_gated` as exact Tier 5 Mike/operator waits.

All 83 work items have undelivered right-to-ignore cards. Report-level closure preserves every sandbox route, authority wait, restart debt, and felt-review state.

## Runtime Observation

The bounded heartbeat window recorded 108 attempts, 108 sends, zero blocks, and zero skip rate. It contained 106 varying consecutive pulses, no near repeats, and no low-component-variance observations. This does not support rescue skipping or a flat generated vector as the cause of Astrid's felt staccato contact in that window, and it does not disconfirm her report or establish another cause.

Connection 1 was current with zero reconnects or disconnects, a 1608.7361 ms first-valid lag, and reliable timing at the bounded capture. The deployed binary predates the additive first-valid spectral-entropy field, so its absent value is restart debt rather than evidence that the initial field had no spectral state.

The Minime point snapshot preserved 67.68% fill, 0.898 spectral entropy, 0.878 resonance density, 0.222 pressure risk, 0.856 viscosity, and the emitted `overpacked_viscous` / `compressed` texture. It is not a causal intervention dossier.

## Alignment And Authority

No live-control authority was granted or inferred. No pressure, fill, PI, controller, rescue, sensory cadence/admission, heartbeat shape/intensity, codec gain, provider, protocol, Minime regulation, correspondence priority, or behavior changed.

Restart alignment is `restart_debt`. Concurrent protocol 1.2 Division, authority, autonomy, and broad Minime runtime edits appeared after this run began. The sanctioned bridge wrapper would compile and deploy those unrelated changes with this field. Their owner must finish or hand off, both repositories must be re-audited, and the combined source/tests must be reviewed before the recorded restart command is safe.

## Verification

- Full isolated bridge library suite: 1,590 passed, 0 failed.
- Strict bridge Clippy: passed with `-D warnings`.
- Bridge-crate formatting: passed.
- Repository-wide formatting: blocked only by a concurrently edited import line in `crates/astrid-minime-protocol/tests/wire_contract.rs`; this run did not rewrite foreign work.
- Focused Minime verification: 3 dynamic-noise pressure-room tests, 1 release-fill epsilon test, 1 extreme-context-cap test, and 86 regulator tests passed before broader concurrent Minime edits appeared.
- Flywheel/control self-tests: Agency Corridor 18, introspection addressing 35, Sandbox queue 27, recent-signal summary 38, proactive scan 110, Evidence Event Store 13, and steward control/projection 30 all passed.
- The earlier sandboxed full bridge run had eight permission-only failures; the isolated 1,590-test pass resolves all eight.

No bridge restart was attempted. This is an ownership and deployment-alignment gate, not a test failure.
