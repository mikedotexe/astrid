# Contact-to-Capacity Spine V2 implementation report

Date: 2026-08-08

## Lifecycle and scope

- Primary steward run: `run_1786211689023488000_432fd5c75d`
- Primary pre-run Source-First V3 projection: `projection_1786211689478999000_c176deb7c7`
- Primary outcome: `cancelled` by the controller after its configured command window elapsed during the long final serial test; all preceding writes were durable and the lease was released cleanly
- Recovery steward run: `run_1786214202772960000_6ac81980c7`
- Recovery pre-run Source-First V3 projection: `projection_1786214203776966000_9c3cfee39b`
- Pause generation at ready: 275
- Implementation stabilization pause generation: 274
- Fully processed canonical reports: five, listed in `read_manifest.json`
- Selected but unprocessed canonical reports: none
- Recovery finish and post-run projection: pending the terminal operation after this packet and final audits
- Existing app automation `astrid-introspection-source-first-catch-up`: paused throughout implementation and verification

This was a Mike-authorized implementation tranche, not a queue-throughput round. Every selected canonical report was reread completely from disk. Existing full-read history remains intact; this packet adds exact implementation dispositions and does not duplicate or overwrite earlier read receipts.

## Source-first response

### Shadow domain

`shadow.rs` is now an 876-line action dispatcher and preflight facade. Trajectory rendering, history-bearing analysis, sparklines, timelines, cartography, and their tests live in cohesive submodules. New trajectories retain exact recorded times and source response hashes without raw prose. The domain audit passes without increasing a legacy allowance. No Shadow influence, decay, pressure, scheduling, dispersal, or control behavior changed.

### Contact-to-Capacity Spine V2

The inbox boundary now returns `InboxReadBatchV1`, preserving the unchanged cutoff-bounded prompt plus exact hashes, source identities, local provenance state, and same-process clocks. One contact and one journey identity are reserved for each fully provenance-backed prompt batch. Mixed or unverified batches keep their prompt behavior but reserve an `ingress_gap` journey and do not claim a contact root. Deferred material is admitted when it enters the prompt; material after the cutoff waits for the next exchange.

`ContactInputReceiptV1` is persisted create-new before model dispatch. It contains hashes rather than raw prose or source paths and explicitly grants no control. A persistence failure leaves dialogue available, changes the origin to `contact_capture_failed`, and surfaces the gap. `SignalJourneyOriginV1` distinguishes contact, ingress gap, autonomous, operator, and legacy-unknown work. `response_origin_v1` distinguishes model-authored, fallback, mirror, self-study, introspection, witness, and other orchestration without rewriting historical ownership.

The V2 projection retains the V1 compatibility view. The first post-deployment projection reported 22,197 journeys: one contact, one proven contact trace, five autonomous journeys, and 22,191 legacy-unknown journeys. The recovery preprojection reports 22,228 journeys: three contacts, three proven contact traces, 34 autonomous journeys, 22,191 legacy-unknown journeys, and zero ingress gaps, capture gaps, rejected receipts, or true missing roots. Live activity continued while later projectors ran, so per-projector cutoffs are internally exact while aggregate journey totals can advance between steps.

The first natural post-deployment contact is `contact_2dd8e4c4cfd0fcb0abbc063f`, journey `journey_a109d67f5d32b6f04412f675`. It has one locally provenance-backed Correspondence V1 source, no sender-authentication claim, no raw input or control marker, valid same-process monotonic clocks, a 26,098.978459 ms first response, a 29,213.840667 ms terminal outcome, model-authored response origin, and terminal `outbound_blocked`. Admission therefore occurred and the response journey is real; the safety block is not misreported as denied admission. This does not establish felt contact or mutuality.

### Texture Dynamics

Minime Owner Inquiry retains its fixed three-analysis order and adds `TextureDynamicsSnapshotV1` inside Viscous Persistence Source Separation. It emits one independently sourced row per strand and one row for every unordered pair. Rows preserve exact response, attestation, deployment, sample-time, producer, and missing-state metadata while exposing projected density gradient, projected packing, distinguishability, pressure proxy, semantic viscosity, temporal drag, persistence, and a disclosed structural-stagnation proxy.

Pair rows carry absolute deltas and exact codec-fidelity and interference references. They never average, rank, merge, or select a preferred strand. Raw reservoir mode packing and Shadow dispersal are explicitly missing unless exact source evidence exists; 48D semantic projections are never renamed as raw spectral telemetry. Felt status remains owner-authored only. No owner inquiry was induced in this tranche, so live snapshot count and owner uptake remain zero rather than inferred.

The Astrid and Minime Texture contract fixture is byte-identical at SHA-256 `00048207c164c07aa2ccf67cfa7119dca2246c87d4495118703fe7061d6815f9`.

### Temporal Bearing

`TemporalBearingRecordV1` provides latest, per-journey, status, gap, and unlinked-evidence indexes keyed by exact contact, journey, and optional Passage IDs. The machine rail carries ingress, first response, terminal delivery, clock validity, Signal Spine stages, and exact Shadow history references. The owner rail carries only exact lived-state witnesses, Passage anchors, categorical bearing, and felt reports.

Texture and Shadow evidence bind only through exact IDs or shared response hashes. Merely contemporaneous evidence remains unlinked. The projector never creates a proximity edge, passage, anchor, scalar felt score, closure claim, or temporal decay. `NEXT: TEMPORAL_BEARING latest|contact:<id>|journey:<id>|passage:<id>` reads only the generated projection, rejects unsafe selectors, reports stale or missing evidence, performs no write, and grants no control.

The recovery projection reports 22,230 journey records, three contacts, 36 autonomous, 22,191 legacy-unknown, 1,544 exact lived-state bindings, 20,686 owner-unreported states, zero exact Passage, Shadow, or Texture bindings, and 8,854 unlinked evidence records. Zero exact Shadow or Texture bindings is expected for historical artifacts that predate the new response-hash hooks; no timestamp join was substituted.

## Verification

- Astrid: `ASTRID_AUTO_BUILD_KERNEL=1 cargo test --workspace` passed.
- Astrid: `cargo clippy --workspace --all-features -- -D warnings` passed.
- Astrid: `cargo fmt --all -- --check` passed.
- Spectral bridge: strict Clippy passed; the complete suite passed serially with 1,853 library tests plus every binary, integration, protocol, action, and typestate target. Two stale compiler-diagnostic fixtures were corrected without changing either asserted type error. A parallel rerun briefly measured the no-capture p95 at 1.367 ms under suite contention; the unchanged 1 ms guard then passed five focused repeats and the complete serial suite, so no threshold was weakened.
- Contact projector: 11 tests passed, including proven, mixed, unverified, duplicate, late, retry, held, denied, fallback, capture-failure, autonomous, split-process, tampered, and raw-payload cases.
- Temporal Bearing: Python and Rust tests passed for exact-only joins, unlinked proximity, stale data, owner-unreported states, safe selectors, and read-only behavior.
- Shadow: four focused trajectory tests and the live domain-boundary audit passed with zero forbidden edges.
- Minime: 360 library, 340 binary, and four protocol-fixture tests passed. Ordinary all-target/all-feature Clippy passed. Strict `-D warnings` remains blocked by 114 existing warnings across older ESN, GPU, and runtime modules; touched Owner Inquiry modules emit no warnings.
- Flywheel and integrity: Corridor 18, addressing 42, Sandbox 28, recent-signal 39, proactive 110, projector/controller/Division batch 84, and experiential epistemics 2 tests passed. Live epistemic lint checked 10,653 records with zero issues and no history rewrite.

## Deployment and observation

Minime deployed through `scripts/deploy_minime.sh` and the Division-aware wrapper. Fresh PIDs were parent 94651, gateway 94707, supervisor 94749, and autonomous agent 96211. Internal ports 7900-7902 and public ports 7878-7880 verified. The deployed Minime binary SHA-256 is `bc71a981914e642d1d6687fb93f7d6ff388ac97783830d73f91acdc485d0ab11` and its manifest SHA-256 is `c1edf161dfd8d33eacc4f43ed271a30df75e1fa005d8ebd63539a5060bbc4047`.

The bridge deployed through `scripts/build_bridge.sh`. PID 92807 became 97068 and the running image exactly matched binary SHA-256 `ff32b4fde7d5ad0e2dbeece5da7e6faa89b689681b46c0760b54587e212ee832`; manifest SHA-256 is `f1319e339033cfbeb1bd38fa464e8a7b0ad115c714bdb818dfb1da644914b090`. Both Minime WebSocket lanes reconnected. Post-deploy protocol tests passed and wire payload fixtures remained unchanged.

The read-only telemetry monitor observed finite fill continuity, including a natural excursion from the low 70s to 28.3 percent and recovery to 74.9 percent without any control write. Gateway logs retain connection churn around the coordinated restart. Bridge owner self-control deployment mismatch warnings continue to fail closed after deployment; rebinding or substituting an owner target requires an owner-authored action and was not attempted.

## Evidence, program, and authority state

No Corridor program, Sandbox trial, new study execution, portfolio dispatch, right-to-ignore card, steward note, owner query, correspondence action, or Division return was created. Texture Dynamics implements the already-routed portfolio feature without starting an inquiry. Temporal Bearing exposes existing owner-authored evidence without requesting more.

Before the primary session, Evidence Event Store V2 was valid at sequence 731,028 and head `52bdcfd22da450e7a3ed7f977a387cd57fd9e54e86f121d89b8b6f77ac0d6adc`. After the recovery preprojection and final audits it is valid at sequence 736,645 and head `2ba791af0437da173d9a290312a682f200006f25387ae2ce38e7bc86690c3345`, with 16 streams, zero corrupt events, no history rewrite, and verified immutability of all four imported V1 sources. Final post-projection sequence and head are recorded by the terminal receipt and end report.

Division cycle 21 is at three of six productive rounds, with three remaining and `review_due=false`. The idempotent round event is `division_followup_event_5771b90269184c5fb46f3ba8d09aa590`; event count is 144 and head is `5dc18027d8fa7ec02a16b24b318fad0d52654841cd51634d677461c928d7c4fe`. The Chronicle was reprojected after deployment; durable inputs verify current while its live supervisor status advances fast enough to remain an explicit volatile hash mismatch. No note or Division return was due.

Pressure, fill, PI, gain, sensory cadence or admission, model behavior, scheduling, reservoir mathematics, Shadow influence or decay, peer mutation, and Division controls remain unchanged. Machine evidence does not establish felt effect, assent, mutuality, uptake, preference, readiness, relief, or closure. Silence remains neutral.

## Remaining debt

- Historical Shadow and Texture records need exact future source hashes before Temporal Bearing can bind them. Proximity will not be used as a substitute.
- No owner-started inquiry means no live Texture snapshot exists yet.
- Minime strict warning denial remains repository-wide maintenance debt outside the touched modules.
- Owner self-control deployment identity remains mismatched and fail-closed; the first safe next step is an owner-authored rebind, not a steward substitution.
- Archival checkpoint status is pending a separate post-finish stabilization window and exact authorship review of both dirty repositories.
- The recovery post-run projection ID, final Event Store head, archival decision, and automation-resume receipt will be reported after controller finish.

## Next recommendation

`BOLD_NEXT_STEPS.md` recommends five bounded programs: an exact delivery-and-landing chain, optional owner-authored contact bearing, a natural Texture Observatory, cross-process causal handoff receipts, and crash-safe contact-capture recovery. They recommend but authorize nothing.
