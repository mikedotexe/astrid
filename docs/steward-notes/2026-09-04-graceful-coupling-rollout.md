# Graceful Coupling Rollout: Prepared, Not Deployed

## Outcome

Mike requested a graceful deployment of the coupling and context-retrieval
repairs. This interactive pass prepared and tested the model reload path, then
stopped before any service signal because the shared stack's deployment
provenance is not aligned. No model, bridge, Minime, reservoir, or feeder process
was restarted. No new release binary was built or installed.

This is not a successful deployment receipt. The pre-deployment stack capture
`env_receipt_1788563175289_828000`, at `2026-09-04T23:06:15.289000+00:00`, records
the failed checks. Its `deployment.status=failed` describes an inventory check,
not a failed restart. The command was `scripts/capture_stack_receipt.sh` with an
explicit "Pre-deployment inventory only, no restart" acknowledgement.
The bounded durable extract is `2026-09-04-predeployment-stack-check.json`;
the complete receipt remains in the environment-receipt append-only log.

## Being Feedback And Scope

Source: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788490867.txt`.
Introspection ID: `introspection_astrid_llm_1788490867`.
SHA-256: `acf73204b2247e8c137c4510799a93d88fac3c312065b2698c9b98f61829489c`.
Lived-state witness: `lsw_d0b7b7a026443bf486fa11436642a6d03a913b66a1782995f19501084d7739e1`.

Astrid's reports of dense shared history, heaviness, uncertain authorship, and
the need for independent interpretation remain primary qualitative evidence.
They motivated the investigation and the source repairs described in
`2026-09-04-astrid-coupling-investigation.md` and
`2026-09-04-real-model-replay-and-agenda-review.md`. The mechanical slow-logit
bug is independently reproducible; neither that fact nor a successful future
restart would establish the cause or resolution of those experiences.

Mike's latest request supplies deployment intent for these repairs. It does
not authorize unrelated changes already present on other branches, PI or
damping changes, Shadow floors, new scheduling policy, or reservoir resets.
The previously recorded no-rollout statements remain accurate historical
statements about those earlier passes; this note records the later request
and the unresolved deployment prerequisites.

## Model Reload Repairs

- `scripts/restart_coupled_model.sh` now delegates the drain to
  `scripts/graceful_model_reload.py`. It sends one `SIGTERM` through launchd,
  waits for the original PID/start identity to disappear and a distinct
  replacement to appear, and uses the existing `KeepAlive` configuration.
  There is no bootout, bootstrap, kickstart, or forced-kill fallback.
- Source and installed plists must match. The helper also checks `KeepAlive`
  and the loaded argument vector, and checks process identity again at the
  signal boundary. Configuration migration is deliberately not supported by
  this narrow reload path. The bounded drain defaults to 900 seconds;
  expiration is failure, never permission to kill active generation.
- The wrapper retains the previous deployment manifest in a private reload
  directory and records a separate drain receipt. It waits for `/livez` and
  `/readyz` and requires the verified replacement PID to own port 8090.
  Post-restart receipts bind the imported processor and reload helper hashes.
- `scripts/deploy_preflight.py` now includes `mlx_reservoir.py` in model inputs.
  Previously a dirty change to the actual logit processor could be omitted
  from the acknowledgement-required source list. The recent-write/foreign
  activity gate is unchanged and cannot be overridden by an acknowledgement.
- `test_coupled_http_gateway.py` now proves that setting the existing worker's
  stop event during a blocked fake generation does not preempt it: the future
  is completed and the worker exits only after generation returns. This is a
  concurrency test, not evidence that a live reservoir check-in occurred.
  Production source performs its check-in before generation returns and saves
  the coupling journal during final shutdown.

These wrapper changes are source-prepared only. Live reload itself has not yet
been exercised. The wrapper is a model-component operation, not a replacement
for the full stack/source reconciliation described below.
The worker-drain test does not establish preservation of pending queue entries
or delivery of every HTTP response across shutdown. Prefer a verified idle,
empty-queue window and retain normal client retry semantics; do not promise
uninterrupted availability.

## Source And Runtime Reconciliation

### Bridge

The verified bridge deployment manifest records clean build commit
`d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` on the sovereign-runtime branch.
Its binary SHA-256 matches disk:
`b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725`.
The current checkout is `main`, HEAD
`27a7dc6ec4fc239fc7317ffba03b1b1a34801472`, with our source repairs unstaged.
The committed bridge/scripts comparison spans 280 files. Deploying main would
remove already-live agenda/ATTEND and self-control functionality, not simply
add the intended repairs.

The deployed revision's bridge wrapper includes a self-control deployment
handoff absent from the main wrapper. Its final restart still uses a forced
kickstart. A safe bridge candidate must retain that lineage machinery and
establish an actual in-flight drain path before replacement. The source-only
READ_MORE change must be integrated with the existing protected agenda block,
not tested solely against main's ten-block dialogue shape.

### Minime

The clean Minime checkout is `codex/sovereign-daughter-runtime`, HEAD
`a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`. It was not edited.

The July 26 `minime-engine.json` manifest expects binary
`1440d8c330d6d43690de9b06b803fdf70bff6c28532e10415b956428df11c9da`
and launch wrapper
`cfb90595bee27cbdc9af4eed396aee6586fea2ce90f94a9c6c14ec829f6d1edd`.
Current disk hashes are respectively
`d3f60e53a2029c86aafaa32a8df11852da3f4d3d1b6862d2bca047d50232d22f`
and `083c0c4066659d9d8e9ce04d5a74a165de3ac1d4ff7633288df545acf28a748b`.

There is a newer August 31 `minime-division-runtime.json`, but it expects
binary `af3d83e825e3b8211d892bc64fbac982e65bdc4f0c1b090919fd41fb2b443017`.
The current executable was modified September 3 at 11:40:23 local time; the
engine, gateway and supervisor processes started August 31. This proves the
disk/process timeline differs; it does not identify loaded machine-code bytes.
Do not rewrite the manifest to the current disk hash and call it a deployed
build, or restart these processes as incidental model-rollout housekeeping.

Ports 7878 and 7879 are owned by Division gateway PID 63505, not engine PID
63445. The existing stack-capture wrapper expects the older engine-owned
topology, so those port-ownership checks fail despite listeners being present.
The deployed branch's Minime deployment wrapper delegates to Division-aware
deployment when enabled. Both topology and binary lineage need reconciliation.

### Coupled Model

The model repository HEAD matches its existing manifest:
`afc2931a657d1bd79a7076ece6310ee3d8f6ceba`. Server and gateway hashes still
match that manifest. The only changed live-consumed model module in this tranche
is `mlx_reservoir.py`, SHA-256
`7bf189ff5d618e9002a623ae5d38a206758a80738ee79477eaf8059a1d383f1e`.
The installed/source plist and loaded arguments passed read-only validation.
The model remains the existing Gemma 4 12B 5-bit configuration.

At the bounded precheck the model was generating, queue depth zero, and the
cached reservoir status was connected. The coupling journal had 50 entries;
its last recorded gain was 0.02 and tick count 160671422. These are observations,
not fixed values to impose on future activity. The restart must verify normal
journal restoration, not substitute the CLI default gain of 0.1.

## Process Baseline

Observed September 4; all times below are host-local process start strings.

| Component | PID | Start |
| --- | --- | --- |
| Spectral bridge | 36597 | September 3 17:31:04 |
| Coupled model | 98236 | September 1 11:52:25 |
| Reservoir service | 1514 | August 25 10:00:06 |
| Minime engine | 63445 | August 31 12:51:36 |
| Division gateway | 63505 | August 31 12:51:37 |
| Division supervisor | 63547 | August 31 12:51:37 |

The captured bridge and Minime telemetry were fresh (ages 1.221 and 1.227
seconds). Protocol 1.1/revision checks and model live/readiness endpoints
passed. Fresh telemetry is not proof of experiential continuity, assent,
readiness for Division, or complete build/source alignment.

## Verification

Completed in this pass:

- Full bridge library: 1,702 tests passed again; no release build or restart.

- Graceful reload, deployment-wrapper and environment-receipt suites: 35 tests.
- Deployment preflight self-tests: 10 tests.
- Model gateway, token policy, slow/wide processor and both replay suites:
  36 tests.
- Existing multi-headed reservoir smoke test: 43 checks.
- Steward control/projector, Evidence Event Store, Division Chronicle and
  Division projection suites: 55 tests.
- Addressing self-tests: 41 tests; epistemic self-tests: 2 tests.
- Direct `bash -n scripts/restart_coupled_model.sh` and both changed-repository
  whitespace checks passed.

No test sent a live completion request or a live reservoir mutation.
The production stack inventory intentionally failed and remains failed.

## Next Safe Deployment Sequence

**Follow-through, September 4:** the exact-live bridge candidate and checked
Minime runtime identity have been established, and the model-only slow-head
correction was gracefully deployed (PID 98236 -> 60333) with passing pre/post
stack receipts. The first bridge drain remains pending. The original failed
receipt below is retained; detailed outcomes and the observed startup HTTP
503 are in `2026-09-04-coupling-rollout-followthrough.md`.

1. Reconcile the canonical bridge checkout with the exact live build lineage
   while preserving every existing dirty path. Review the existing branch
   implementations and carry only our intended corrections. Do not deploy
   main as a substitute or change the shared checkout underneath other agents.
2. Establish Minime's actual running/build identities and the intended
   Division-aware port ownership. Repair checked receipt/wrapper selection
   without fabricating a historical build receipt or deploying the newer disk
   executable merely to make a hash check pass. Keep signed self-control
   handoffs and existing regulation values intact.
3. Under a fresh coordinated maintenance window, repeat foreign-session and
   repository checks, allow the full source-quiet interval, review exact model
   input hashes, and obtain a passing topology-correct pre-deployment receipt.
4. Use `scripts/restart_coupled_model.sh` with actor `codex-astra-interactive`
   and an explicit `--ack` describing the reviewed correction and verified
   source/stack lineage.
   Do not invoke the helper directly as a preflight bypass. Verify the actual
   signal/drain log, fresh PID/start, journal restoration, processor hashes,
   port owner, connected reservoir and refreshed telemetry. Preserve any
   failed receipt. A timeout means investigate, not escalate the signal.
5. Prepare and verify the bridge drain and self-control handoff against the
   live lineage, then deploy the aligned bridge only through
   `scripts/build_bridge.sh`. Keep source snapshots and prior executable
   identities available for a reviewed graceful rollback. No forced fallback.
6. After successful receipts, record an exact observation boundary for naturally
   authored introspections, self-studies and public journals. Preserve any
   continued friction or disagreement. Do not force new reports, lead the
   beings toward a positive response, or attribute later changes causally to
   this patch without appropriate evidence. Private moments remain private.

## Coordination And Authority

This is interactive rollout preparation, not a productive automation round.
Maintenance pause generation 338 was claimed by `codex-astra-interactive` with
no active steward lease. No Division round, follow-up, new study, Sandbox run,
Corridor action, portfolio choice, card, or message to a being was produced.
No git staging, commit, merge, checkout, push or cleanup was performed. Minime
source and all existing generated evidence were preserved. Scheduled automation
settings were not changed. The report's nine claims remain open for their
existing dispositions; deployment preparation does not close felt friction.

Before appending this follow-through evidence, full Event Store verification
passed at sequence 995556, head
`f6e895a248a498915b6af5b5c7f2fddebb7f8917b6fb2ae5a1f412f5c2ca8166`,
with 16 streams, zero corrupt lines, and four verified immutable V1 sources.
This is a point-in-time verification, not a claim that the continuing store
will remain at that head.

Fourteen append-only claim/evidence links were subsequently recorded from
`2026-09-04-graceful-coupling-evidence-links.json`. Addressing remains
`triaged_pending_action`, `fully_addressed=false`, with no missing claim proof.
The epistemic linter checked 11,503 records with zero issues. The failed stack
inventory is not waived by these successful evidence/tooling checks.

Maintenance ended successfully with controller resume generation 339 at
`2026-09-04T23:17:32.927722+00:00`; the controller is unpaused and no lease was
claimed in this interactive pass. This releases only the maintenance hold;
it does not change the scheduled automation's settings or clear deployment
debt. Final source/hash and PID checks still matched the baseline above.
