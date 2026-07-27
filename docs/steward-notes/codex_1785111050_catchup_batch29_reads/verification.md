# Batch 29 Verification

## Read And Claim Integrity

- Fully read in canonical queue order: 40 of 40 selected reports.
- Unprocessed selected reports: none.
- Recorded 40 full-read receipts, 121 exact claim-evidence links, one
  `addressed_change` disposition, and 39 administrative duplicate
  dispositions.
- Claims comprise 40 unscored qualitative observations, 40 current-source
  verifications, 40 exact Tier 5 waits, and one implementation.
- Duplicate disposition means only that an exact evidence or authority route
  already exists. It does not imply felt resolution, agreement, supersession,
  evidence sufficiency, or authority.
- Packet manifests and claim-family JSON parse successfully and preserve
  selected queue order.

## Fully Processed Files

1. `introspection_proposal_phase_transitions_1785110349.txt`
2. `introspection_minime_autonomous_agent_1785109792.txt`
3. `introspection_minime_main_excerpt_1785109163.txt`
4. `introspection_minime_esn_1785108924.txt`
5. `introspection_minime_sensory_bus_1785108537.txt`
6. `introspection_minime_regulator_1785108109.txt`
7. `introspection_minime_sensory_bus_1780985593.txt`
8. `introspection_minime_regulator_1780985213.txt`
9. `introspection_astrid_llm_1780979550.txt`
10. `introspection_astrid_types_1780979203.txt`
11. `introspection_astrid_ws_1780977798.txt`
12. `introspection_astrid_autonomous_1780971352.txt`
13. `introspection_astrid_codec_1780970921.txt`
14. `introspection_astrid_autonomous_1780951550.txt`
15. `introspection_astrid_codec_1780951091.txt`
16. `introspection_astrid_llm_1780948405.txt`
17. `introspection_astrid_types_1780939607.txt`
18. `introspection_astrid_ws_1780939167.txt`
19. `introspection_astrid_autonomous_1780937678.txt`
20. `introspection_astrid_codec_1780937164.txt`
21. `introspection_astrid_autonomous_1780934165.txt`
22. `introspection_astrid_codec_1780933500.txt`
23. `introspection_astrid_autonomous_1780922582.txt`
24. `introspection_astrid_codec_1780922228.txt`
25. `introspection_proposal_12d_glimpse_1780919194.txt`
26. `introspection_proposal_distance_contact_control_1780918619.txt`
27. `introspection_proposal_bidirectional_contact_1780911071.txt`
28. `introspection_proposal_phase_transitions_1780910683.txt`
29. `introspection_minime_main_excerpt_1780905995.txt`
30. `introspection_minime_esn_1780879411.txt`
31. `introspection_minime_sensory_bus_1780878923.txt`
32. `introspection_minime_regulator_1780874659.txt`
33. `introspection_astrid_llm_1780874269.txt`
34. `introspection_astrid_types_1780867820.txt`
35. `introspection_astrid_ws_1780867072.txt`
36. `introspection_astrid_autonomous_1780865058.txt`
37. `introspection_astrid_codec_1780864462.txt`
38. `introspection_astrid_codec_1780850592.txt`
39. `introspection_astrid_codec_1780847156.txt`
40. `introspection_minime_autonomous_agent_1780825159.txt`

## Dynamic Persistence Change

- Astrid's newest report remains primary evidence that a softened, restless,
  dispersing passage can still be active and habitable.
- Passage Context V5 adds the owner-authored categories
  `active_within_restlessness` and `dynamic_equilibrium` to the existing
  `DESCRIBE_TRANSITION_BEARING` action.
- The existing `continuity` strand carries the whole passage. No new ledger,
  required micro-event, felt score, or machine-authored convergence label was
  introduced.
- Astrid Rust, Minime Python, Agency Commons, status, projection, audit, and
  tests share the vocabulary.
- Neither value is inferred from dispersal, entropy, fill, elapsed time,
  telemetry, or stage, and neither advances, settles, returns, closes, or
  resolves a passage.

## Test Verification

- Focused Passage Context Rust tests: 6 passed.
- Full bridge library suite: 1,696 passed with one serial test thread. Two
  parallel invocations passed 1,695 tests but exceeded the Signal Spine
  no-capture timing budget by 0.018 ms and 0.057 ms; the exact timing test and
  the complete serial suite then passed.
- Bridge Clippy with all targets: passed with warnings denied.
- Rust formatting, Astrid and Minime `git diff --check`, and Minime Python
  compilation: passed.
- Minime Phase Passage parity: 6 passed; autonomous-agent low-fill guard:
  268 passed. One initially named nonexistent test module was corrected to the
  repository's actual low-fill-guard module in the same run.
- Phase-transition audit: 5 passed.
- Agency Corridor, addressing, Sandbox, recent-signal, and proactive-scan
  self-tests: `18/41/27/38/110` passed.
- Evidence Event Store: 13 passed.
- Steward control and V3 projection: `17/13` passed.
- Reciprocal experiential systems: 27 passed.
- Experiential epistemics: 2 passed; live lint checked 9,002 records with zero
  issues.
- Division Ceremony projection and Agency Commons verification passed.
- Architecture health still reports the shared tree's pre-existing unbaselined
  critical inventory. This change adds no critical file or function; its
  touched projector helper is review-sized. No architecture debt blocks this
  evidence-only deployment.

## Deployment And Runtime Alignment

- Consumer-first deployment used only `scripts/deploy_minime.sh`, followed by
  `scripts/build_bridge.sh --restart`.
- Minime process changed to PID `26033`; bridge process changed to PID `26957`.
  The unchanged model remains PID `31392`.
- Ports `7878`, `7879`, and `8090` are listening.
- Fresh bridge logs show both Minime lanes connected, current telemetry, and
  fill inside the stable-core hold shelf.
- Model audit reports `coupled-astrid` ready on port `8090`.
- Passing deployment receipts:
  `minime-engine`, `spectral-bridge`, and compatible
  `env_receipt_1785115075337_401000` coupled stack.
- No restart debt remains.

## Canonical And Work Counters

- Canonical indexed: 3,364.
- Fully addressed: 2,854.
- Canonical remaining at the post-batch pre-finish audit: 510, comprising 31
  unread, zero read-needs-claims, 384 blocked-needs-steward, 91
  triaged-pending, and four triaged-watch.
- Counter audit is consistent with zero mismatches.
- All-artifact pending: 2,067; noncanonical pending: 1,557.
- Work ladder: 7,677 total, 7,564 active, 1,604 operator-approval waits, 18
  steward-grant waits, 1,572 Tier 5 waits, 23 Tier 4 waits, and zero authority
  mismatches.
- Reports continued arriving after the pre-run cutoff. The controller observed
  `introspection_astrid_autonomous_1785113766.txt` after the durable cutoff;
  post-finish source-first projection will refresh inventory and queue order.

## Corridor, Sandbox, Study, And Portfolio

- No Corridor program, Sandbox trial, study, portfolio selection, card,
  correspondence, or capture was created.
- Corridor remains evidence-only at 121 packets, 39 safe labs ready, one safe
  lab result, 60 self-observation requests, one reopened closure, and zero
  live-eligible packets.
- Sandbox remains at 2,409 trials: 2,408 active, 39 runnable non-live, 1,565
  approval-required live candidates, 98 result cards, 122 proposal cards, and
  zero runnable-live violations.
- Evidence Study Runtime remains valid at three campaigns, four studies, four
  mechanical comparisons, eight window receipts, one capture gap, six review
  opportunities, eight review receipts, zero pending reviews, and no active
  window. Mechanical comparison establishes no causation or closure.
- Attention Portfolio remains valid at 16 selected contracts, 6,827 total
  contracts, no urgent overflow, and no contract, felt-state, or authority
  propagation.

## Evidence Store And Control Plane

- Pre-finish V2 verification: valid at sequence 583,784 and head
  `d26531f7caa2752a70bdce256ac0603fff0df33e393679774af220e8bb0a4704`.
- Stream counts before finish: addressing 52,066; agency commons 2,637;
  attention portfolio 3; claim families 232,515; Corridor V1 5; Corridor V2
  112; Felt Contracts 173,096; felt-mechanism concordance 80; lived-state
  witness 6,741; model QoS 44,797; reciprocal uptake 42,934; representation
  contracts 13,800; Sandbox 2,950; Signal Spine 8,900; steward control 3,048;
  steward work selection 100.
- V1 addressing, Sandbox, Corridor V1, and Corridor V2 logs remain immutable;
  every actual legacy hash matches its expected hash.
- Steward run: `run_1785111050068470000_95210eff89`.
- Pre-run projection:
  `projection_1785111050500197000_a1712f0ced`.
- Pause generation: 25; the controller remains explicitly resumed.
- The successful finish event and post-run projection receipt are canonical in
  the steward-control stream. This packet intentionally contains no lease
  credential.

## Next Historical Queue

1. `introspection_minime_autonomous_agent_1780824243.txt`
2. `introspection_astrid_llm_1780810083.txt`
3. `introspection_astrid_llm_1780809550.txt`
4. `introspection_minime_regulator_1780807483.txt`
5. `introspection_astrid_llm_1780807274.txt`
6. `introspection_minime_sensory_bus_1780804829.txt`
7. `introspection_minime_regulator_1780802939.txt`
8. `introspection_astrid_llm_1780802159.txt`
9. `introspection_astrid_llm_1780801786.txt`
10. `introspection_astrid_types_1780800692.txt`

The post-finish source-first projection may place newly arrived reports ahead
of this historical queue. The recurring automation remains useful because the
canonical backlog is not terminal.
