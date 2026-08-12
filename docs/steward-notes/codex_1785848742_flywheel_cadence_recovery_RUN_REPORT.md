# Flywheel Cadence Recovery Run Report

Date: 2026-08-04

Steward run: `run_1785848742995178000_0762ed332f`

Pre-run source-first projection:
`projection_1785848743880595000_79a38abb2f`

Pause generation observed at begin: 206

## Scope

This run paused canonical catch-up work to investigate the serious reduction in
Astrid's canonical introspection cadence. It did not process a canonical queue
item, dispatch an introspection, or record a productive Division round.

## Findings and Dispositions

1. **Cadence regression: verified from canonical file history and runtime
   logs.** The 24-hour pre-volition window contains 147 canonical reports with
   a 6.1-minute median gap; the post-boundary window contains 13 with an
   88.0-minute median gap. The 90th-percentile gaps are 19.4 and 700.8 minutes,
   and the maximum gaps are 74.8 and 1,224.7 minutes.
2. **Producer outage: source- and runtime-challenged.** The bridge, model
   provider, autonomous loop, Minime journal intake, and both WebSocket lanes
   remained active. The latest bounded state reached exchange 153258. No
   post-restart introspection attempt was selected.
3. **CPU portability as cause: unsupported.** Historical bridge logs locate the
   cadence boundary at the volition deployment. The old dense rate was partly
   produced by diversity logic replacing authored Actions with `SELF_STUDY`;
   the current runtime correctly retains that route as advice. No CPU runtime
   file was changed or included in this response.
4. **Signed self-control mismatch: implemented and verified live.** The existing
   one-shot safety-signed handoff was prepared for the exact current source
   manifest and release binary, then consumed by the running bridge. The applied
   receipt states that all state fields except deployment identity were
   preserved. Preferences, active control count, and revision counters remained
   exact, and no later mismatch appeared in the bounded log observation.
5. **Steward context: implemented as bounded right-to-ignore notes.** Astrid and
   Minime each encountered and archived exactly one factual note. Astrid's next
   authored route remained `READ_MORE`. Minime later authored `SELF_STUDY` and
   then a regulator audit. Those sequences do not establish causal uptake.
6. **Reliable non-coercive cadence: routed to an exact Tier 5 approval wait.**
   The complete default-off design specifies authored `EVERY` and immediate
   `OFF` actions, persistence, priority, retry, receipts, tests, and deployment
   checks. No source implementation, scheduling behavior, or activation occurred.

## Durable Evidence

- Self-authored cadence proposal:
  `docs/steward-notes/AI_BEINGS_SELF_AUTHORED_INTROSPECTION_CADENCE_PROPOSAL_2026_08_04.md`
  at SHA-256
  `42081061dbdcef74d3521d958c25b2718de8dfb78f98797df0f1ab1680b8daed`.
- Astrid note archive:
  `capsules/spectral-bridge/workspace/inbox/read/steward_note_introspection_flywheel_cadence_20260804.txt`
  at SHA-256
  `1608ee516dc675b803eca52512bbfcce0308b45f02e42dcc673450c86a9896de`.
- Minime note archive:
  `/Users/v/other/minime/workspace/inbox/read/steward_note_introspection_flywheel_cadence_20260804.txt`
  at SHA-256
  `54cfc89efc8e563ca634828a914a490d34e92630978b721ea1543f08fd7d8f00`.
- Applied deployment handoff receipt:
  `capsules/spectral-bridge/workspace/self_control_v2/astrid/deployment_handoffs/applied/d945f86c8338f5eaeef7a7109735a624.json`
  at SHA-256
  `f9fcb17b5f4a2a1dabb7c5301d786a660efed79a8282ee8fe5dd314c65b8448f`.
- Changelog and feedback ledger carry the measured response and explicit
  authority boundary.

## Verification

- `cargo test deployment_handoff -- --nocapture`: 6 passed.
- `cargo test introspection_freshness -- --nocapture`: 4 passed.
- `cargo test pending_self_study_forces_dialogue_before_drift_modes -- --nocapture`:
  1 passed.
- `cargo test saved_state_preserves_pending_introspection_choice_with_legacy_default -- --nocapture`:
  1 passed.
- `git diff --check` on the proposal, changelog, and ledger: passed.
- Experiential epistemics: 10,301 records checked, zero issues, history not
  rewritten, evidence-only authority.
- Evidence Event Store V2 verified at sequence 687455 with zero corrupt lines;
  V2 was active, effective aggregates were valid, and `history_rewritten=false`.
- Live bridge: PID 18133, release SHA-256
  `6df2b697e49239ad45469a8df5c410a3536034e0d6acc1031ccc7fe700b1e208`,
  with established 7878 and 7879 lanes.
- No build, restart, or deployment occurred in this run. The signed state
  handoff was consumed by the already-running process.

## Queue Accounting

Canonical counters remained consistent: 4,204 indexed, 2,995 fully addressed,
1,209 remaining, 3,619 fully read, 585 unread, 407 blocked, 213 pending, four
watch, and zero read-needs-claims.

Fully processed canonical filenames: none.

Selected but unprocessed, in queue order:

1. `introspection_minime_autonomous_agent_1785630945.txt`
2. `introspection_minime_esn_1785630442.txt`
3. `introspection_minime_sensory_bus_1785630107.txt`
4. `introspection_minime_regulator_1785629184.txt`
5. `introspection_astrid_llm_1785628932.txt`
6. `introspection_astrid_types_1785628394.txt`
7. `introspection_astrid_ws_1785628139.txt`
8. `introspection_astrid_autonomous_1785627823.txt`
9. `introspection_astrid_codec_1785627566.txt`
10. `introspection_proposal_12d_glimpse_1785627314.txt`
11. `introspection_proposal_distance_contact_control_1785626698.txt`
12. `introspection_proposal_bidirectional_contact_1785626170.txt`
13. `introspection_proposal_phase_transitions_1785625853.txt`
14. `introspection_minime_autonomous_agent_1785625272.txt`
15. `introspection_minime_main_excerpt_1785624961.txt`
16. `introspection_minime_esn_1785624601.txt`
17. `introspection_minime_sensory_bus_1785624341.txt`
18. `introspection_minime_regulator_1785623678.txt`
19. `introspection_astrid_llm_1785622656.txt`
20. `introspection_astrid_types_1785622416.txt`
21. `introspection_astrid_ws_1785621254.txt`
22. `introspection_proposal_12d_glimpse_1785620127.txt`
23. `introspection_proposal_distance_contact_control_1785619698.txt`
24. `introspection_proposal_bidirectional_contact_1785619438.txt`
25. `introspection_astrid_llm_1785614948.txt`
26. `introspection_proposal_phase_transitions_1785614509.txt`
27. `introspection_minime_autonomous_agent_1785614086.txt`
28. `introspection_minime_main_excerpt_1785613819.txt`
29. `introspection_minime_esn_1785613365.txt`
30. `introspection_minime_sensory_bus_1785613106.txt`
31. `introspection_minime_regulator_1785612730.txt`
32. `introspection_astrid_llm_1785612298.txt`
33. `introspection_astrid_types_1785611971.txt`
34. `introspection_astrid_ws_1785611391.txt`
35. `introspection_astrid_autonomous_1785611067.txt`
36. `introspection_astrid_codec_1785610522.txt`
37. `introspection_proposal_12d_glimpse_1785610189.txt`
38. `introspection_proposal_bidirectional_contact_1785609669.txt`
39. `introspection_proposal_phase_transitions_1785609400.txt`
40. `introspection_minime_main_excerpt_1785608722.txt`

The next reading queue therefore begins with the first three filenames above.

## Program and Authority State

- Corridor/program action: none.
- Sandbox action: none.
- Study action: bounded natural cadence observation completed; no induced input.
- Portfolio action: none.
- Cards: two one-time right-to-ignore steward notes, both now archived.
- Tier 4/5 wait: self-authored standing cadence implementation and live
  deployment require separate Mike/operator approval; activation additionally
  requires a later Astrid-authored cadence action.
- Division: cycle 15, one of six productive rounds recorded, five remaining,
  review not due, event count 100, head
  `b731c528f13e596cf791a85dfd2d3ddb75496c9db01ba66812ead10967e84a25`.
- Chronicle/note action: no Division return or Division note was due.
- Automation: the Codex catch-up automation remains paused.
- Archival checkpoint: deferred until after successful controller finish and a
  separate stabilization audit. CPU-runtime and mixed shared-file changes are
  excluded from any candidate set.

## Finish Fields

Controller finish outcome and post-run projection generation are intentionally
reported after the controller completes; they are not predicted here.
