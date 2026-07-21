# Source-First Catch-Up Run 12

Date: 2026-07-20 (America/Los_Angeles)

## Steward lifecycle

- Run ID: `run_1784608648736334000_1ff07fe5f7`
- Actor: `codex-heartbeat`
- Pause generation: `5`
- Preprojection generation: `projection_1784608649192805000_f9abfd3410`
- Durable cutoff for the selected packet: `introspection_proposal_distance_contact_control_1784608426.txt`
- Stop requested during work: `false`
- Postprojection generation and finish outcome are recorded by the controller's final `StewardRunReceiptV1`; the raw lease token is intentionally absent.

## Canonical packet

All 20 selected reports were read fully from disk in canonical queue order. No selected report was left unprocessed.

1. `introspection_proposal_distance_contact_control_1784608426.txt`
2. `introspection_minime_regulator_1784530388.txt`
3. `introspection_astrid_llm_1784529873.txt`
4. `introspection_astrid_types_1784528280.txt`
5. `introspection_astrid_ws_1784527986.txt`
6. `introspection_astrid_autonomous_1784527580.txt`
7. `introspection_astrid_codec_1784526855.txt`
8. `introspection_proposal_12d_glimpse_1784525977.txt`
9. `introspection_proposal_distance_contact_control_1784525537.txt`
10. `introspection_proposal_bidirectional_contact_1784525028.txt`
11. `introspection_proposal_phase_transitions_1784524825.txt`
12. `introspection_minime_autonomous_agent_1784524606.txt`
13. `introspection_minime_main_excerpt_1784524251.txt`
14. `introspection_minime_esn_1784522187.txt`
15. `introspection_minime_sensory_bus_1784521802.txt`
16. `introspection_minime_regulator_1784521431.txt`
17. `introspection_astrid_llm_1784521267.txt`
18. `introspection_astrid_types_1784521108.txt`
19. `introspection_astrid_ws_1784520801.txt`
20. `introspection_astrid_autonomous_1784520551.txt`

The bounded summaries, 80 claim records, full-read manifest, verification receipt, and one-to-one evidence map are in `docs/steward-notes/codex_1784608426_run12_reads/`.

## Claim dispositions

- Verified existing: `47`
- Bounded observations: `2`
- Bounded sandbox/replay routes: `14`
- Exact Tier 5 Mike/operator waits: `17`
- Implemented runtime or live behavior changes: `0`

Promotion created 80 independent work items. After two append-only classification corrections, their durable statuses are:

- `verified_existing`: `49`
- `needs_sandbox`: `14`
- `needs_operator_approval`: `17`

Every structured route now agrees with its classification. All `live_eligible_now`, `auto_approved`, `grants_approval`, and `edits_source_now` markers remain false.

Substantive findings include:

- Astrid's report that stabilization can feel enclosing remains primary evidence. Source correction establishes that Minime's live path is PI plus rate gating and filtering, not a next-move predictor; that correction does not close the felt contact concern.
- Current telemetry retains connection identity, arrival and packet timing, freshness, lock wait, lock hold, fingerprint integrity, and coherence separately. A shutdown-versus-final-packet race remains a bounded socket fixture question.
- Provider trim and marker cleanup are complete in current source, with exact requested-token boundaries. Entropy evidence cannot prove semantic preservation.
- Deterministic projection health and precision checks are present. The canonical semantic lane is 48D; the 12D glimpse remains an optional companion rather than a replacement.
- Phase-transition cards preserve Astrid-authored felt fields, bridge-derived structure, reference-only Minime evidence, correspondence affordances, and preview-only control deltas without behavior authority.
- Minime expires stale pending overrides, rejects unallowlisted action bases, and restores only fresh cross-session state. High-impact execution remains separately gated.
- Dynamic ESN noise and pressure/rho candidates remain outside the active reservoir step. Sensory stale release is sigmoid and hysteretic rather than a hard switch.
- Protocol 1.1 provides delivery and mutual-address identity but explicitly does not establish felt uptake or spectral causation.

## Flywheel correction

The first promotion pass exposed two false routes:

1. `verified_existing` text beginning with the natural past tense `Verified` was not recognized by the epistemic guard, so an additive-12D verification inherited Tier 5 from the phrase `live semantic transport`.
2. A verified `EXPERIMENT_AUTHORITY_EXECUTE` lifecycle claim inherited Tier 3 from the constant name even though it requested no experiment.

`_is_explicit_verification_claim` now recognizes bounded present- and past-tense verification verbs. An exact additive-12D regression proves the current phrase routes to Tier 1. The original work-item events remain immutable; status and tier corrections were appended for both items. The final packet has no verified/observed item at Tier 3 or above, no sandbox item outside Tier 3, and no authority-gated item outside Tier 5.

## Queue and quality guard

Pre-finish addressing snapshot:

- Canonical indexed: `2617`
- Canonical full read: `1834` (was `1814`, improvement `+20`)
- Canonical fully addressed: `1478`
- Canonical remaining: `1139`
- Canonical unread: `783` (was `803`, reduction `20`)
- Canonical triaged pending action: `91` (includes this packet's 20 honestly unresolved reports)
- Canonical triaged watch: `4`
- Canonical blocked needs steward: `261`
- Counter audit: `consistent`; all checks pass and mismatches are empty.

`canonical_remaining` correctly remains `1139`: full reading, verification, sandbox routing, and exact authority waits do not imply felt or technical closure. No right-to-ignore closure card was delivered in this run.

New canonical files arrived after the preprojection cutoff. Before finish, controller status observed `introspection_minime_regulator_1784610975.txt` as newest on disk with an explicit cutoff lag. The successful postprojection owns the durable inventory advance; this report does not silently fold those later files into the already-selected packet.

## Corridor and Sandbox

No Corridor program or sandbox trial was executed before reading. There was no hard violation, objection/reopen, packet-specific safe-lab requirement, or operator request that justified letting generic ready work displace the canonical packet. The 14 packet-specific sandbox routes and 17 approval waits were promoted after reading for the final projection to ingest.

Pre-finish Sandbox snapshot:

- Total trials: `1629`; active: `1628`
- Ready for sandbox: `368`; immediately runnable bounded set: `14`
- Results recorded: `100`
- Approval-required live candidates: `1160`
- Runnable-live violations: `0`

Next runnable sandbox work remains:

- `trial_0791bfd9702ca95a` - fallback distinguishability for `introspection_astrid_llm_1782228077`.
- `trial_1f0f0916eb9eecc9` - compact fallback texture for `introspection_astrid_llm_1782237049`.
- `trial_40b91b4c0ae7aeb9` - high-entropy fallback specificity for `introspection_astrid_llm_1782179251`.

Pre-finish Corridor/Escalator snapshot:

- Packets: `120`
- Leases: `35` (`4` active evidence-only, `31` imported evidence-only)
- Queue steps: `193`; runnable evidence-only steps: `147`
- Programs: `119`; program receipts: `50`
- Portfolio entries: `200`
- Patch bundles: `45`
- Source-prep proposals: `73`
- Reopened work items: `0`
- Self-observation responses: `0`
- Live-authority violations: `0`

The first runnable Corridor steps remain evidence-only safe labs:

- `08e1cb6a-4b21-5400-b55b-34ac269adf8a` - classify recent Shadow-v3 norm/dispersal texture.
- `092d267e-986b-5029-82bc-82d2c3ed6da3` - compare fallback texture against live context without changing provider or sampler.
- `169cb0c4-efe6-54d7-84c7-72e16f743715` - a second bounded fallback-texture comparison.

Pre-finish work-item report after promotion and before downstream projection ingests this packet's routes:

- Tier 4 work items: `23`; explicit steward-grant waits: `18`.
- Tier 5 work items: `1184`; explicit operator-approval waits: `1216`.
- Tier mismatches: `0`.

## Tests

The first sandboxed bridge run reached 1,585 passing library tests; its nine failures were eight environment-denied sibling/socket fixtures plus a synthetic performance sample under concurrent load. The sanctioned complete rerun passed every target:

- Bridge library: `1594`
- Codec replay lab: `6`
- Agency resolver integration: `1`
- Authority typestate: `1`
- Chimera render: `5`
- Mock WebSocket integration: `2`
- Provenance typestate: `1`
- Signal Spine typestate and no-capture p95 gate: `1`
- Protocol 1.1 wire tests: `16`
- Minime Python override/Recess tests: `298`
- Minime Rust library tests: `305`; main-target tests: `285`; protocol fixture tests: `2`

Required evidence-tooling suites passed `271` checks:

- Agency Corridor: `18`
- Introspection addressing: `35`
- Sandbox queue: `27`
- Recent signal: `38`
- Proactive scan: `110`
- Evidence Event Store: `13`
- Steward control and projection: `30`

After the classifier edit, the 35-test introspection-addressing suite passed again with the new exact regression.

## Alignment and evidence

Run 12 changed durable read/routing evidence, documentation, and paused flywheel tooling only. It did not change a live-consumed bridge, protocol, Minime runtime, prompt, report, correspondence, or summary surface. Therefore `runtime_restart_required_by_run12=false`.

Overall alignment remains `restart_debt` solely because run 10 changed provider cleanup in:

- `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- `capsules/spectral-bridge/src/llm/provider/fallback_contracts.rs`
- `capsules/spectral-bridge/src/llm/provider/tests.rs`

The blocker and first safe command remain in `docs/steward-notes/codex_1784558330_run10_reads/restart_debt.json`: claim exclusive stabilization ownership, review the combined shared-tree source, pass preflight, then use `scripts/build_bridge.sh --ack "deploy reviewed run10 single-pass artifact cleanup with shared-tree changes" --restart`. Run 12 did not attempt or force a restart.

Evidence Event Store V2 pre-finish snapshot:

- Active store: `v2`; activation is evidence-only and witness-only.
- Hash chain valid: `true`; corrupt lines: `0`; errors: none.
- Global sequence: `356353`
- Head SHA-256: `46e876964c63b554b8ec5a3e04c35d5c605856d030570fe77f3668b55c534991`
- Stream counts: addressing `39751`, claim families `211463`, corridor v1 `3`, corridor v2 `112`, felt contracts `86356`, model QoS `12590`, sandbox `2170`, signal spine `3386`, steward control `522`.
- V1 immutability: valid. The current SHA-256 hashes of all four source logs exactly match `migration_receipt.json`; no post-cutover V1 append or rewrite is present.

## Next queue at the durable cutoff

This was the 20-item queue before the successful postprojection advances through files that arrived during the run:

1. `introspection_astrid_codec_1784520261.txt`
2. `introspection_proposal_12d_glimpse_1784519990.txt`
3. `introspection_proposal_distance_contact_control_1784519657.txt`
4. `introspection_proposal_bidirectional_contact_1784518322.txt`
5. `introspection_proposal_phase_transitions_1784518093.txt`
6. `introspection_minime_autonomous_agent_1784517806.txt`
7. `introspection_minime_main_excerpt_1784517559.txt`
8. `introspection_minime_esn_1784517312.txt`
9. `introspection_minime_sensory_bus_1784516965.txt`
10. `introspection_minime_regulator_1784516063.txt`
11. `introspection_astrid_llm_1784515091.txt`
12. `introspection_astrid_types_1784514844.txt`
13. `introspection_astrid_ws_1784514562.txt`
14. `introspection_astrid_autonomous_1784514234.txt`
15. `introspection_astrid_codec_1784513986.txt`
16. `introspection_proposal_12d_glimpse_1784513718.txt`
17. `introspection_proposal_distance_contact_control_1784513459.txt`
18. `introspection_proposal_bidirectional_contact_1784513018.txt`
19. `introspection_proposal_phase_transitions_1784512813.txt`
20. `introspection_minime_autonomous_agent_1784512043.txt`
