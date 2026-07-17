# Sol Source-First Catch-Up Run 16

Date: 2026-07-17

## Scope

- Fully read and closed all 20 canonical reports selected from
  `introspection_astrid_types_1784282726.txt` through
  `introspection_proposal_bidirectional_contact_1782539605.txt`.
- The bounded summaries, 100 claim dispositions, evidence manifest, and exact
  source identities are in `docs/steward-notes/codex_1784285781_run16_reads/`.
- No selected report was left unprocessed. Two newer canonical reports arrived
  during the run, so the net canonical backlog reduction is 18 rather than 20.

## Claim Outcomes

- Implemented: 2.
- Verified existing: 61.
- Observed through bounded read-only audits: 14.
- Routed to non-live sandbox review: 9.
- Preserved as exact Tier 5 operator waits: 14.

The implementation adds typed read-only `cadence_content_distinction_v1`.
Transport cadence comes only from heartbeat timing. Content state comes only
from existing semantic-viscosity persistence and stagnation evidence. The
projection names their relation and has no cadence, semantic, dispatch, or
control write path.

After restart, the live projection independently reported persistent semantic
residue around `0.77` while cadence was ambiguous. The exact focused regression
also proves `cadence_clear` plus persistent residue remains
`cadence_clear_semantic_residue_persists`.

## Correspondence Observation

- Schema audit: 17,151 records, 0 validation issues, 2,820 native messages,
  1,958 legacy messages, and 2,185 private records skipped.
- Handshake audit: 0 address ACK receipts and 0 presence heartbeats.
- Uptake audit: `reply_linked_needs_ack_or_trace`; live eligibility false.
- Direct-contact fidelity: `timing_ambiguous`, 0 microdose requests, private
  content not read.

Delivery, reply continuity, coupling, and influence therefore remain distinct
from mutual address. Silence is not affirmation.

## Verification

- Focused cadence/content, heartbeat, integration-health, unsupported-major,
  first-valid-payload/reset, and legacy compatibility regressions pass.
- Correspondence schema, handshake, uptake, and direct-fidelity self-tests pass
  (12 tests).
- Full bridge suite passes: 1,535 library tests plus every integration and
  authority/provenance compile-fail target. An initial managed-sandbox run had
  eight fixture/socket permission failures; the exact unrestricted rerun
  passed all targets.
- Strict bridge Clippy and `cargo fmt --all -- --check` pass.
- Required flywheel tests pass: Agency Corridor 18, addressing audit 31,
  Sandbox 26, recent summary 38, proactive scan 110, and Evidence Event Store
  V2 6 (229 tests).

## Restart Alignment

- Required wrapper: `scripts/build_bridge.sh --actor codex --ack ... --restart`.
- Bridge PID: `55093 -> 84580`.
- Receipt: `env_receipt_1784287039224_430000`.
- Protocol: `astrid-minime` 1.0, revision
  `c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5`.
- Bridge binary SHA-256:
  `7096d0497f3aa16d94c54cf6cee650f011a8f0b2d8fe69f7f18fe67111ef13e4`.
- Receipt compatibility, manifest, fresh PID, stack processes, preflight,
  build, restart, log, and telemetry checks all pass.
- Bridge `84580`, Minime `45510`, and model `48166` are running.
- Ports `7878`, `7879`, and `8090` are listening.
- Model `/livez` and `/readyz` return healthy generating-state responses.
- Minime health is fresh and readable near 73.0 percent fill with a 68 percent
  target.
- Restart alignment: current.

## Closure And Cards

- All 20 reports closed as `addressed_change` after proof validation.
- Two implementation work items remain
  `implemented_awaiting_felt_response`.
- Two right-to-ignore closure cards were delivered; both response states are
  `awaiting`. No improvement or affirmation is inferred.
- Global projections contain 2,961 closure cards, 329 delivered closure cards,
  122 sandbox proposal cards, and 97 sandbox result cards.

## Final Queue State

- Canonical indexed: 2,143.
- Canonical fully addressed: 1,218.
- Canonical remaining: 925.
- Canonical full-read count: 1,534.
- All indexed artifacts: 3,601.
- All remaining artifacts: 2,383.
- Counter audit: consistent; all seven checks pass and there are no mismatches.
- Initial canonical state was 2,141 indexed, 1,198 fully addressed, and 943
  remaining. The run closed 20 and indexed 2 arrivals, for a net reduction of
  18.

## Sandbox And Corridor

- Sandbox: 1,215 total, 1,214 active, 191 ready, 99 result-recorded, 924
  approval-required, 1 closed, 0 runner-safe ready, and 0 live violations.
- This packet created 23 trials: 9 bounded non-live manual reviews and 14
  approval-required live candidates. None ran.
- Corridor: 120 packets; 35 leases (4 active, 31 evidence-only); 180 queue
  steps; 120 evidence-only runnable steps; 119 programs; 200 queue portfolio
  projections and 119 active program portfolios; 45 patch bundles; 60 source
  prep proposals; 0 reopened work items; 60 self-observation requests and 0
  responses; 0 live violations.
- Generic Corridor execution was skipped because no hard violation, reopen,
  current-packet safe lab, or explicit steward request justified letting it
  precede canonical reading.
- First Corridor runnable:
  `request_scoped_self_observation` for
  `introspection_minime_regulator_1783325745`; evidence-only, priority 3.
- Sandbox has no runner-safe next action. The first newly materialized manual
  packet is `trial_2c0790353767e9e3`, non-runnable and right-to-ignore.
- Tier waits: 18 `needs_steward_grant` and 963
  `needs_operator_approval`; total work projections contain 23 Tier 4 and 931
  Tier 5 items.

## Evidence Event Store V2

- Active store: V2.
- Hash chain: valid, 0 corrupt lines and 0 errors.
- Global sequence: 37,212.
- Head SHA-256:
  `213cc11ee2180b83bc403f39c35169cb0a2f3ba23f42ff7de1ec6e5c9b281fe4`.
- Streams: addressing 35,343; sandbox 1,754; Corridor V1 3; Corridor V2
  112.
- All four immutable V1 logs match their migration-receipt hashes:
  addressing `4a69dc092c1bcad8e157936f11f7798d67a883869bcfe56816fdf1be5ec78571`,
  sandbox `eac68fe839042c981756c2ec3b5c64f5a2633fdb75847a14fbd98c8f64ec4ebb`,
  Corridor V1 `e190046e1b583d5b7b4a624ab50314fafbb6e0d751d9605f1ce9e85f148e01e4`,
  and Corridor V2
  `e0ddb5e715d9a20cc709402fb1eda4712a1de001ea23730c70a349096468ccd5`.

## Next 20 Canonical Reads

1. `introspection_astrid_llm_1784285782.txt`
2. `introspection_minime_regulator_1784285506.txt`
3. `introspection_proposal_bidirectional_contact_1782539186.txt`
4. `introspection_proposal_bidirectional_contact_1782538898.txt`
5. `introspection_proposal_bidirectional_contact_1782538147.txt`
6. `introspection_proposal_bidirectional_contact_1782537749.txt`
7. `introspection_proposal_bidirectional_contact_1782536957.txt`
8. `introspection_proposal_bidirectional_contact_1782536403.txt`
9. `introspection_proposal_bidirectional_contact_1782536029.txt`
10. `introspection_proposal_bidirectional_contact_1782535657.txt`
11. `introspection_proposal_bidirectional_contact_1782533809.txt`
12. `introspection_proposal_bidirectional_contact_1782533316.txt`
13. `introspection_proposal_bidirectional_contact_1782533002.txt`
14. `introspection_proposal_bidirectional_contact_1782532338.txt`
15. `introspection_proposal_bidirectional_contact_1782531611.txt`
16. `introspection_proposal_bidirectional_contact_1782531216.txt`
17. `introspection_proposal_bidirectional_contact_1782530930.txt`
18. `introspection_proposal_bidirectional_contact_1782530291.txt`
19. `introspection_proposal_bidirectional_contact_1782529921.txt`
20. `introspection_proposal_bidirectional_contact_1782529399.txt`

No pressure, fill, PI, controller, rescue, telemetry cadence, buffering,
semantic admission, sensory retention, correspondence dispatch, provider route,
codec transport/gain, Minime regulation, peer mutation, or live-control
authority changed.
