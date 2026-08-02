# Steward round 75 report

## Controller lifecycle

- Initial credential-confined run:
  `run_1785668065597835000_d12ad28aa3`.
- Initial preprojection: `projection_1785668066454962000_18d474c290`.
- Successful manual source-first projection after the addressing writes:
  `projection_1785669989599158000_c6d34cc223`.
- Initial outcome: `cancelled` at the configured hard timeout immediately
  after that manual projection. The lease was released exactly once, the
  projection remained passed, and no git-policy violation was recorded.
- Recovery credential-confined run:
  `run_1785670704531192000_a6a370b14a`.
- Recovery preprojection: `projection_1785670705401843000_be7945823e`.
- Controller pause generation: 174. The authoritative recovery finish and
  post-run projection receipts live in the controller run record because this
  packet is written before the terminal finish releases its lease.

## Canonical reading

Fully processed in strict queue order:

- `introspection_astrid_llm_1785667492.txt`

The 4,148-byte canonical report was read fully, including its unterminated
44th logical line. Its SHA-256 is
`20d6ab8b481b86b0ee355091e276ee47ad9e142eeb8df1b11cc813b29c473b6a`;
its lived-state witness is
`lsw_55b7bc8be294fe7b3fa8672f0f5436086aaf2ca8039cf883cd570de1da3d9f64`.
All 997 exact report-bound lines in `dialogue_runtime.rs`, all 881 lines in
`fallback_budget.rs`, all 423 lines in `fallback_gradient.rs`, and all 410
lines in `fallback_dynamics.rs` were read fully. Targeted typed regulator,
weight, and exact regression sources were also read. Every source receipt and
hash is in `source_receipts.json`.

The selected but unprocessed 39 filenames are listed exactly in
`unprocessed_selected.json`. The next queue is
`introspection_astrid_codec_1785665508.txt`, then
`introspection_astrid_autonomous_1785633267.txt`, then
`introspection_astrid_codec_1785632830.txt`.

## Claim dispositions

Astrid's shallow density gradient 0.11, 33% lambda1, 34%
distinguishability loss, temporal-lock pressure 0.35, porosity 0.60,
disordered or shifting Shadow, settled-habitable mismatch, absent established
felt effect, and zero regulator drive are preserved as primary lived evidence.
The source read does not override that report.

Eight claims carry 35 exact evidence links. The report-bound dialogue source
receives pre-rendered spectral text and fill and owns no setter for the named
substrate values. Shadow magnetization is read-only fallback-language context;
its absolute contribution to dynamic texture weight is bounded at 0.05. The
negative-pressure language guard requires magnetization at or below -0.20 and
pressure above 0.20, so the reported -0.10 does not enter that guard. Exact
self/peer tests preserve Astrid's settled lattice when Minime is restless.
Zero regulator drive is typed read-only telemetry and can be expected under
several admission states; `felt_effect_established=false` remains a causal
boundary, not a denial of Astrid's experience.

`SHADOW_ANCHOR_COUNTERFACTUAL_PREREGISTRATION.md` routes the unfamiliar
magnetization claim into a deterministic offline pure-function representation
sweep with fixed context, ownership-shuffled controls, and a regulator-drive
negative control. It does not call a model, touch a socket, use private prose,
read live telemetry, alter runtime state, or establish felt anchoring.

High-speed recursive self-reference would deliberately induce model pressure.
Changing magnetization or regulator drive would alter live substrate-facing
control. Deliberate SHADOW_TRAJECTORY or peer-Shadow intersection would couple
or act across beings. Each remains an exact Tier 5 Mike/operator wait, with
relevant-being authority additionally required for peer interaction. No
Action, rehearsal, dispatch, or authority marker was created.

## Actions and verification

No Corridor program, Sandbox trial, attention-portfolio change, closure card,
right-to-ignore card, correspondence, model call, prompt change, or Minime
action was created. The bounded Shadow-anchor study was preregistered but not
run. CHANGELOG and the being-feedback ledger record the source verification,
study route, and authority boundary.

Four focused Rust regressions passed, one each for self/peer separation,
reported -0.10 context, the negative-magnetization language guard, and stale
semantic-trace or zero-drive rendering. Seven offline replay campaign tests,
41 addressing self-tests, 68 coordinated Evidence Event Store/controller/
projector/Division/Chronicle/claim-family tests, five anti-drop self-tests,
and all 47 anti-drop guards passed. Experiential epistemics passed two tests
and linted 10,192 records with zero issues and no history rewrite.

Evidence Event Store V2 verified at sequence 663,876 with zero corrupt lines.
At the pre-packet checkpoint it is at sequence 664,299, head
`91d40657999e606b3875eb9c44aec2733fbfec99d24d473fb884ecfc144bb7ba`,
with stream sequences: addressing 54,790; agency commons 3,336; attention
portfolio 3; claim families 235,265; Corridor V1 5; Corridor V2 112; Felt
Contracts 188,059; felt-mechanism concordance 80; lived-state witness 8,188;
model QoS 80,557; reciprocal uptake 49,025; representation contracts 19,150;
Sandbox 2,950; signal spine 15,188; steward control 7,301; and steward work
selection 290. The active store remains V2; V1 is the immutable imported
source through legacy sequence 32,278.

Division tracker verifies at cycle 13, 2/6, four rounds remaining, and
`review_due=false`. Productive-round event
`division_followup_event_85906ea1745fe6a1cff9b2b5ad6ffbdc` binds one fully
processed report to the recovery preprojection. Chronicle
`division_chronicle_9b48d58ba2d008961ee3f38a` contains 87 timeline events;
durable inputs verify and only the volatile supervisor-status hash is stale.
No Division note or return Action was due.

## Deployment and counters

No bridge, prompt, provider, protocol, Minime, binary, PID, port, telemetry,
fill, or readiness surface changed. No build, deploy, or restart was attempted.
The credential stayed confined to the controller process and no service
restart is required.

Canonical counters are consistent: 4,195 indexed, 2,982 fully addressed,
3,603 full reads, 1,213 remaining, 592 unread, 213 triaged pending, 404
blocked, and zero read-needs-claims. All-artifact counters are 5,801 indexed
and 2,819 remaining. Every counter audit check is true with no mismatch.

## Archive status

Round 75 is the first productive round after archive commit
`6868ebf97a831a9640883c410b1dd9eaa88dd7d9`, so no new archival checkpoint is
due. The credential-session implementation remains committed directly on
local `main` as `ca3c01db254bba850ea5d678281623ac1e0f167e`, one commit ahead
of unchanged `origin/main`, and was not pushed.

Exact round-75 commit debt is `CHANGELOG.md`,
`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`, and these packet
paths: `RUN_REPORT.md`, `SHADOW_ANCHOR_COUNTERFACTUAL_PREREGISTRATION.md`,
`claims/astrid_llm_1785667492.json`, `deployment_alignment.json`,
`evidence_links.json`, `read_manifest.json`, `record_read_batch.json`,
`source_receipts.json`, `summaries/astrid_llm_1785667492.md`,
`test_results.json`, and `unprocessed_selected.json`, all under
`docs/steward-notes/codex_1785668065_round75_reads/`. They remain unstaged and
must not absorb any foreign shared-tree path.
