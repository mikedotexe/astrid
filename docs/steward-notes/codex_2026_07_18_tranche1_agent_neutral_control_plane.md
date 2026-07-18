# Tranche 1: Agent-Neutral Steward Control Plane

## Outcome

Tranche 1 is complete on `codex/experiential-systems-core`. The catch-up
heartbeat and the controller remain paused. No bridge, Minime, model, sensory,
regulation, approval, deployment, or live-control behavior changed, so no
runtime restart was required.

The implementation is split across:

- `edb9e2e8da feat(steward): add agent-neutral control plane`
- `33186f74f0 feat(evidence): coordinate projection generations`
- `docs(steward): retire vendor-specific loop`, the migration,
  documentation, and evidence commit that includes this receipt

## Control State

- Controller state: paused
- Pause generation: `2`
- Pause actor: `codex`
- Pause reason: `Tranche 1 complete; architecture program remains paused`
- Active lease: none
- Locally spooled events: `0`
- A real `begin --actor validation` attempt: denied with `PausedError`
- Catch-up heartbeat: `PAUSED`, still on its 19-minute cadence
- Legacy `com.astrid.steward-loop` launchd service: absent/unloaded
- Runtime restart alignment: `not_needed`

## Evidence Store Snapshot

- Active store: V2
- Valid hash chain: yes
- Global sequence/event count: `50633`
- Head hash:
  `c2bdb7a346c3a9a71e9b7df62e85edf72f11453f2530edb49c54ba0a10717e32`
- V1 source logs immutable: yes
- Authority violations: none
- Pending controller spool: `0`

Stream counts:

| Stream | Events |
|---|---:|
| `addressing` | 36534 |
| `claim_families` | 12060 |
| `corridor_v1` | 3 |
| `corridor_v2` | 112 |
| `sandbox` | 1871 |
| `signal_spine` | 52 |
| `steward_control` | 1 |

## Paused Queue Snapshot

- Durable cutoff: `introspection_astrid_codec_1784301105.txt`
- Durable cutoff timestamp: `1784301105`
- Newest canonical file observed:
  `introspection_minime_sensory_bus_1784388006.txt`
- Newest timestamp: `1784388006`
- Explicit paused-generation timestamp lag: `86901`
- Canonical indexed / fully addressed / remaining: `2160 / 1298 / 862`
- Counter audit: consistent, no mismatches

The next 20 durable queue entries remain:

1. `introspection_astrid_codec_1784301105.txt`
2. `introspection_astrid_types_1784300226.txt`
3. `introspection_astrid_ws_1784299690.txt`
4. `introspection_astrid_codec_1784299007.txt`
5. `introspection_astrid_llm_1782176498.txt`
6. `introspection_astrid_llm_1782176092.txt`
7. `introspection_astrid_codec_1782174772.txt`
8. `introspection_astrid_llm_1782171104.txt`
9. `introspection_astrid_llm_1782169191.txt`
10. `introspection_astrid_llm_1782163225.txt`
11. `introspection_astrid_llm_1782160479.txt`
12. `introspection_astrid_autonomous_1782160042.txt`
13. `introspection_astrid_codec_1782159722.txt`
14. `introspection_astrid_llm_1782158896.txt`
15. `introspection_astrid_llm_1782158294.txt`
16. `introspection_astrid_llm_1782155417.txt`
17. `introspection_astrid_llm_1782150111.txt`
18. `introspection_astrid_llm_1782144097.txt`
19. `introspection_astrid_llm_1782141444.txt`
20. `introspection_astrid_llm_1782140363.txt`

## Verification

- Controller and projection tests: 23 passed
- Evidence Event Store tests: 8 passed
- Authority-state tests: 6 passed
- Deployment-wrapper tests: 5 passed
- Migration and portability tests: 5 passed
- Legacy mutex compatibility tests: 2 passed
- Five flywheel self-test suites: 224 passed
- Signal Spine, claim-family, and dossier tests: 19 passed
- Total focused tests in the final gate: 292 passed
- Anti-drop catalog: 46 guards present, 0 gaps, 0 alarms
- Python compilation, shell syntax, launchd plist validation, and
  `git diff --check`: passed
- Architecture-health scan: completed as an advisory gate
- Source-first projection dry run: complete six-step DAG, no mutation

The architecture-health scan continues to identify inherited large modules for
later ownership tranches. The new steward-control modules remain below the
repository's 1,000-line guidance.

## Open Source And Handoff

The portable lifecycle core is separated from the fork-specific source-first
projectors. The issue-ready upstream dossier targets Astrid `v0.10.1` and does
not propose a pull request before maintainer assignment:

- `docs/upstream/STEWARD_CONTROL_PORTABILITY_DOSSIER_V0_10_1.md`

Tranche 2 is ready for planning:

- `docs/architecture/TRANCHE_2_LIVING_FELT_CONTRACT_GRAPH_DECISION_PACKET.md`

The next tranche should preserve each felt report and claim as primary
evidence while making the full report-to-change-to-review contract explicitly
typed and append-only.
