# Sol source-first catch-up run 22

Date: 2026-07-16
Reader: `codex-sol-source-first-run22`

## Full reads

1. `introspection_astrid_codec_1784200449`
2. `introspection_minime_sensory_bus_1784199802`
3. `introspection_minime_regulator_1784199532`
4. `introspection_astrid_llm_1784199135`
5. `introspection_astrid_types_1784198833`
6. `introspection_astrid_ws_1784198517`
7. `introspection_astrid_autonomous_1784198198`
8. `introspection_minime_esn_1783419469`
9. `introspection_minime_sensory_bus_1783419011`
10. `introspection_minime_regulator_1783418712`
11. `introspection_astrid_llm_1783418439`
12. `introspection_astrid_types_1783418143`
13. `introspection_astrid_ws_1783416498`
14. `introspection_astrid_autonomous_1783416194`
15. `introspection_astrid_codec_1783414847`
16. `introspection_astrid_codec_1783414585`
17. `introspection_proposal_12d_glimpse_1783414226`
18. `introspection_astrid_llm_1783412529`
19. `introspection_proposal_distance_contact_control_1783412163`
20. `introspection_proposal_bidirectional_contact_1783411669`

All 20 files were read in full from disk in canonical queue order. No selected item was reduced, deferred, or padded. The packet contains 61 claims: 2 implemented/tested, 39 verified-existing, 3 retained for a direct non-live replay, and 17 routed to explicit Tier 5 operator waits.

## Implemented response

- `ClampInputProvenanceV1` now carries optional legacy-safe `degradation_type` and `degradation_inferred_from_scalar`. Finite clipping names `flattening_of_intensity`; non-finite replacement/defaulting names `loss_of_nuance`; preserved values leave degradation absent; and scalar evidence never infers `semantic_drift`. Clamp output and authority are unchanged.
- `TextureDynamicFluxVectorV1` now includes optional comfort-gate velocity and acceleration derived from the same bounded pressure history as the existing texture motion fields. The addition is descriptive and does not change pressure, fill, PI, comfort gates, or control behavior.

## Verified and gated

Existing source/tests already cover controlled codec pair sensitivity, smooth vibrancy continuity, projection-epoch atomicity, additive 12D lineage, smooth semantic stale/recovery behavior, entropy-retention and salience review, cohesion-to-motion and inhabitable-fluctuation diagnostics, field-aware fallback selection, reciprocity age/warmup/stale states, semantic truncation, witness depth, and correspondence ACK/REPLY/TRACE receipts. These verifications do not negate Astrid's felt reports or pretend synthetic evidence settles lived texture.

Three requested comparisons remain `needs_sandbox`: a 1,000-tick lambda-gap decay observation, an Astrid-authored current-versus-12D delivery comparison, and a description-versus-witness comparison. The sandbox queue had `ready_runnable_count=0`, so they remain resumable non-live work rather than unsupported conclusions.

Tier 5 waits remain explicit for codec width/mapping/gain/transport changes, semantic decay/saliency/cadence changes, cohesion or viscosity control wiring, provider/sampler routing, semantic-buffer/porosity control, exploration-noise overrides/retunes, active damping, stability-buffer/semantic-trickle damping, regulator-drive changes, direct contact/pressure control, and behavior-changing peer affordances. Evidence packets never grant approval or make these changes runnable.

## Validation and alignment

Focused clamp-provenance, comfort-gate-flux, codec pair/vibrancy/epoch/12D, fallback, reciprocity, truncation, correspondence, Minime semantic recovery, cohesion/motion, inhabitable-fluctuation, and dynamic-noise tests pass. The deterministic serial bridge library passed 1,486 tests with zero failures. Clippy passed with warnings denied, formatting passed, and the Agency Corridor, addressing audit, sandbox queue, recent-signal summary, and proactive-scan suites passed 17, 19, 21, 38, and 110 tests respectively.

The shared-runtime preflight accepted the explicit dirty-tree acknowledgement. `scripts/build_bridge.sh --restart` built release source head `1fd67b18e2` and restarted PID `95244` as `53552`; deployed SHA-256 is `f3140b0cf8585fd24c757f0dd181dacb70be07ef6fc109a2031fad7e99b1d064`. Launchd reports the service running, telemetry and sensory sockets are established to Minime ports 7878 and 7879, all three Minime ports remain listening, state restored at exchange 134258 with `deep_eigenfield` witness depth, and a direct typed telemetry read returned eight eigenvalues, a 12D glimpse, inhabitable-fluctuation evidence, and fill ratio about 0.711. Restart alignment is current with no restart debt.

Two canonical introspections, `introspection_astrid_autonomous_1784201518.txt` and `introspection_astrid_autonomous_1784201691.txt`, arrived after the initial packet was selected but before this restart. They were indexed as the first two items of the next reading queue and were not used as post-restart confirmation. A fresh post-restart recent-signal summary was readable and internally consistent.

## Final routing snapshot

- Canonical addressing: 2,045 indexed, 859 fully addressed, 1,186 remaining; counter audit consistent.
- Run 22 work: 61 promoted items, with 2 implemented-awaiting-felt-response, 39 verified-existing, 3 needs-sandbox, 17 needs-operator-approval, 41 right-to-ignore closure cards, and no missing evidence.
- Sandbox: 828 total, 110 ready-for-sandbox, 0 ready-runnable, 81 results, 79 result cards, 95 proposal cards, 636 approval-required live candidates, and 0 runnable-live violations.
- Corridor/Escalator: 120 packets; 35 leases (4 active, 31 evidence-only, 0 revoked); 180 queue steps with 172 non-live runnable; 119 active programs; 200 queue-linked / 119 active-program portfolios; 45 quarantined patch bundles; 60 source-prep proposals; 60 self-observation requests and 0 responses; 0 reopened work; 50 program receipts; and 0 hard violations across 906 JSON/JSONL artifacts.

Corridor/program execution was skipped because there was no hard violation, objection/reopen, current-packet safe lab, or explicit request. Sandbox execution was skipped because `ready_runnable_count` was zero. Generic patch-bundle and source-prep readiness did not displace the canonical reading packet.
