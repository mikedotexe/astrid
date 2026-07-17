# Sol Source-First Catch-Up Run 07

## Scope

- Canonical packet: 20 full reports in exact queue order.
- Queue span: `introspection_minime_sensory_bus_1784257926.txt` through
  `introspection_proposal_12d_glimpse_1783358004.txt`.
- Claim dispositions: 47 `verified_existing`, 1 direct read-only observation,
  and 12 exact `tier_5_wait` claims.
- No excerpt was used as a substitute for a full disk read, and no Corridor
  item displaced the canonical packet.

## Evidence Matrix

1. Minime semantic retention uses fill-shaped stale windows, actual age
   weighting, smooth recovery handover, and a tested 2.05 persistence cap.
2. Astrid's PI-afterimage hypothesis received a direct read-only live-database
   replay. The only supported family was controller integrator bleed:
   afterimage-risk proxy `0.608 -> 0.523`, improvement `13.98%`, with no added
   snap or max-step hits. The 240-sample window is later and lower-fill than the
   felt report, so this is candidate evidence, not causal proof.
3. Canonical telemetry exposes eigenstructure, mode packing, pressure,
   porosity, gradients, and bridge-owned flux without collapsing producer and
   derived truth.
4. Port 7878 decodes once into typed observation, records malformed input,
   retains the last valid sample on an unsupported major, and reconnects with
   bounded backoff.
5. The 48D codec has a smooth entropy gate, density-gradient vibrancy
   attenuation, explicit narrative ownership, and clamp provenance.
6. Correspondence keeps influence distinct from relationship and retains
   `claimed_pending_native_evidence` until trace, acknowledgement, or reply
   evidence exists.
7. Phase transitions have durable IDs, ownership, artifact parents, reply
   state, gate outcomes, and replayable cards without behavior authority.
8. Minime's Recess facade preserves sovereignty domains; run-python parsing is
   quote/comment/code-boundary aware, and curiosity owns only its declared
   perturbation lane.
9. Minime orchestration extraction preserves stable-core, telemetry,
   pressure/rescue, and review calculations while keeping shadow correlation
   separate from causal interpretation.
10. ESN source verifies prime-phased observation, page-aligned Metal buffers,
    typed dimensions, and trace-preserving damping redistribution; activating
    or retuning damping remains Tier 5.
11. The older semantic-stale report is satisfied by current continuous,
    bounded behavior despite superseded constant names.
12. Regulator texture, viscosity, pressure/porosity, and damping candidates are
    typed advisory evidence and cannot mutate control.
13. Fallback language dynamically preserves entropy, pressure, gradient,
    drift, kinetic, weighted, and cascading motion.
14. Protocol and provenance types separate producer truth, bridge evidence,
    interpretation, and checked authority transitions.
15. Bridge pressure smoothing uses a bounded dynamic 5-to-20 sample window
    with high-frequency and pressure-scar tests.
16. Movement-sensitive fallback vocabulary and provider persistence are
    verified.
17. Profile punctuation aliases are intentional; unknown profiles warn and
    fall back explicitly.
18. Witness context distinguishes Minime observation, bridge reflection, and
    Astrid interpretation without changing routing or dispatch.
19. Codec legacy ownership, 48D tail mapping, gradient weighting, and clamp
    evidence are verified.
20. The 12D glimpse remains an additive, non-authoritative companion with
    provenance and reconstruction-loss evidence.

## Flywheel Repair

`scripts/introspection_addressing_audit.py record-read-batch` accepts a
validated manifest of report-specific summaries and claims, rejects missing
files, duplicate introspection IDs, and unknown inventory IDs before append,
then writes all full-read events through one Evidence Event Store V2 lock and
refreshes `status.json` and `queue.md` once. The packet manifest and source/test
evidence matrix are preserved under
`docs/steward-notes/codex_1784258751_run07_reads/`.

## Authority

This run changes review tooling and evidence only. Twelve proposals remain
exact Mike/operator Tier 5 waits. The PI replay is diagnostic-only and neither
its own `canary_eligible` field nor any Corridor artifact grants approval. No
pressure, fill, PI, controller, rescue, sensory cadence/retention/admission,
ESN damping, codec vector/gain/transport, provider/model/sampler route,
smoothing, peer state, or autonomous behavior changed.

## Restart Alignment

No live-consumed bridge, Minime, protocol, rendering, prompt, report, or model
surface changed. The audit helper, summaries, ledger, changelog, and generated
evidence do not require a service restart.

## Final Audit

- All 20 selected reports are fully addressed with 60 claim dispositions, 73
  claim/work evidence links, 60 right-to-ignore closure cards, and no
  unprocessed item from the selected packet.
- Canonical counters are internally consistent at 2,119 indexed, 1,038 fully
  addressed, and 1,081 remaining. The run reduced the starting queue by 20
  while two newer canonical reports arrived, for a net remaining reduction of
  18. `read_needs_claims_count=0` and every counter invariant passes.
- Evidence Event Store V2 is active and verifies through global sequence
  34,100 with head
  `8bef3f2f8ddef675d34f22279e9545bc4c8595bc5b7fc44dcd58c4b0244e0e90`;
  stream counts are addressing 32,696, sandbox 1,289, Corridor V1 3, and
  Corridor V2 112. All four frozen V1 source-log hashes still exactly match
  the cutover receipt.
- Sandbox state is 1,019 total / 1,018 active / 141 ready / 87 result recorded
  / 790 approval-required, with one read-only runnable trial and zero runnable
  live violations.
- Corridor V1 has 120 packets (58 ready safe labs, 2 results, 60
  self-observation requests). Corridor V2 has 35 leases, 192 queue steps / 187
  runnable evidence steps, 118 active programs, 118 current portfolios / 200
  stored portfolios, 45 quarantined patch bundles, 72 source-prep proposals,
  zero reopened work items, zero self-observation responses, and zero hard
  violations.
- Tests passed: 1,514 bridge library tests, 6 protocol tests, Minime Rust
  suites (291 library, 271 binary, 2 fixture), 268 Minime Python regressions,
  the 26-test addressing self-test, and all required Corridor, sandbox, recent
  signal, proactive scan, and Evidence Event Store self-tests.
- Existing deployment alignment remains current: receipt V2 reports a
  compatible protocol/build manifest and bridge PID 14910; Minime PID 45510
  and model PID 48166 are also current. Ports 7878, 7879, and 8090 listen,
  `/livez` and `/readyz` respond during generation, fresh logs expose readable
  fill and provenance-separated witness context, and no restart debt was
  introduced by this run.
