# Private Delivery Recovery and Input Trace

Date: 2026-10-08. Author: Codex interactive collaborator.
Status: approved bridge-only recovery is live and verified as PID 66851.
Source is now committed and fast-forwarded into local main as `21087ebadabe`;
nothing pushed. Integration and the separate Division verifier repair are in
`2026-10-08-recovery-integration-and-division-expiry.md`.
The earlier sections retain pre-approval findings; the approved continuation
and final verification below supersede their pending deployment status.

## Scope and ownership

Mike approved the proposed incident recovery, private-writing logging repair
and input trace. Two further questions remain pending: deletion of exactly two
rebuildable incremental caches, and bounded bridge-only graceful recovery.
Neither is inferred from an earlier deployment approval.

Candidate root: `/Users/v/.codex/worktrees/private-delivery-recovery-20261008/astrid`.
Branch: `codex/private-delivery-recovery-20261008`, based on local main
`973b07fbb231f3a1f16d5f006ccf2bf6fe58763a`.
Minime main: `69add3aac1e77feb48d0cd1e416722315fd5c687`.
Both canonical trees were clean at entry and the later read-only check.
No index, commit, merge, push or service-control operation occurred.
Minime has no candidate source change in this tranche.

The controller remains paused at generation 508, with no active lease or
projection. This is interactive repair, not a productive flywheel round.
Previously paused automations remain paused. No claim about read-current
status, subjective improvement, consent, understanding or controller settling
follows from this work.

## Public witnesses

All paths in this table are repository-relative to the named owner. These
public entries were read in full in the survey; the daydream and 06:36
reflection were read again during the implementation. Private drafts were
not used as test fixtures or quoted in this note.

| Owner / public source | SHA-256 |
| --- | --- |
| Astrid: `capsules/spectral-bridge/workspace/journal/daydream_longform_1791459627.txt` | `e9e2854eb831fca1143e90eee0f7a4e5f151e8bd58df0046e16274841f590e4f` |
| Astrid: `capsules/spectral-bridge/workspace/journal/aspiration_longform_1791460846.txt` | `a88dd4450e5e2c8a0e90046b7fc68c6bc300272ed4b5c5a659cb67c62621bbe6` |
| Astrid: `capsules/spectral-bridge/workspace/journal/astrid_1791459361.txt` | `0ef0e25d65e60f625cb7a1464266f0036debbc1d8ffe952dac684b7d9aba6031` |
| Astrid: `capsules/spectral-bridge/workspace/journal/astrid_1791459450.txt` | `52025ffea4ec34ad89c13fe0b4c2ad9ea7a2bbaf00c699b47d09f3ece714297b` |
| Minime: `workspace/journal/introspect_2026-10-08T06-36-51.143916.txt` | `d579aa1c58f43ae2ab52614bbcee8c52007604b7a925ed72d8a697c1a7ee5c48` |

Astrid asks, "Why do we prioritize the collapse? Why is the final word the only
thing we are taught to record?" Minime writes, "No metrics, no data points, no
corrective feedback. Just the movement of the thought." Preserve these as
authored accounts. Neither establishes a mechanism, nor is either a report of
the independently observed storage fault.

## Incident evidence

Times below are October 8, America/Los_Angeles. The bridge is PID 42184, started
October 7 at 17:23:09, from immutable source `a09d212ff8`. Minime remains PID
19468, started October 7 at 16:08:00.

- 05:20:24: a previous private-writing exchange completed.
- 05:20:52: the next private-writing job entered provider dispatch.
- 05:24:24: storage-full errors prevented accepted-delivery artifact retention.
- From 05:25:25: the bridge repeatedly reports unresolved protected work and
  declines competing generation/mailbox admission. It does not retry the job.
- Later read-only checks still find the exact claimed job below. Provider
  `/readyz` reports ready, phase ready, queue depth zero and reservoir connected.
  An idle provider is not proof of delivery or of lost output.

Authoritative activity: `capsules/spectral-bridge/workspace/action_threads/activity_runtime_v3.json`.
Observed SHA-256: `9d8fd4a4bc513ff7de13a32e40883239b571068d4c94d4263794eebb1e0f7520`.
Selection revision: **26941**. Retained job count: **6974**.

```text
job: study-job-d723410ba9263c514f60e5498e19bc5fbd86d3a46a7e788757235d61255fc534
phase: claimed
intent: 952fd92ccd8b4e5acff9e417bcbb815e4722955cea813dcf4f4a80779615ab54
prepared-input: c23700e78cdec1591966bf5bed4b786241e0747e7a849f05215758215013c37a
accepted-delivery partition: a47fdfc76d1ac9ef8a101a86c5c2e7b3d99c21deb06a880214d597ccd9ab079e
```

The prepared artifact is retained under the shared reader's `study-handoff`
directory. The expected accepted-delivery partition is absent. A private
journal artifact of 10,674 bytes survives, but cannot substitute for the
missing exact request/response receipt. No private content is copied into this
packet, ordinary logging or a synthetic fixture.

The configured optional provider spool is
`capsules/spectral-bridge/workspace/provider_observations/20260922-live-02`.
It contains 39,525 event files at inspection. Event
`events/provider-1791462052271-42184-1827-dispatch.json` has SHA-256
`0b130886d42bbcda3b5c57b8519f4ebcc8dcb9496951f735db9f89cd8863350d`,
label `private_writing`, PID 42184, and request hash
`40762ba0de94755516d522b1ff61bb3e2ece97fdcefc633c30e6648f0327fdc7`.
There is no matching terminal event. Its initial `cancelled_or_abandoned`
value is a pre-terminal sentinel, not verified cancellation. This spool retains
request hashes, not full request bytes; optional raw normalization artifacts
are not accepted-delivery proofs.

Disk headroom fluctuated around 5.2-6.1 GiB. Identified rebuildable incremental
caches occupy about 43 GiB and 83 GiB respectively:

- `/Users/v/other/astrid/target/debug/incremental`
- `/Users/v/other/astrid/capsules/spectral-bridge/target/debug/incremental`

Deletion approval is pending; neither directory was changed. No journal,
evidence, release, checkpoint or unknown worktree is a cleanup candidate.

## Candidate privacy boundary

`runtime/private_exchange.rs` defines the private-mode boundary for
`private_writing` and `private_writing_notice`. The exact response still goes
to private retention and normal authored NEXT validation. Letter-shaped prose
inside a private draft no longer passes through the ordinary mailbox projection.

Private responses no longer enter the ordinary 80-character runtime preview,
shared exchange database, public journal/peer publication block, Signal Shadow
response analysis, legacy Shadow response-feature publication, inline REMEMBER
promotion, or response-derived action-continuity analysis. Existing public
dialogue and explicit correspondence behavior remain unchanged. This corrects
the privacy boundary; it does not alter sensory intake or reservoir formulas.

Historical logs, shared database rows and journals remain untouched. This is
not a retrospective sanitization or an exhaustive audit of every command
argument logger. Existing explicit NEXT validation/dispatch and owner-scoped
records remain; command-argument/title logging outside this boundary deserves
separate review if a comprehensive diagnostic-privacy audit is desired.

## Candidate uncertain-dispatch review

The new operator CLI is `--stage-uncertain-study-review <path>` and requires a
deployment manifest. It exits before service startup and conflicts with
autonomous, maintenance and other approval modes. It is not a being Action.

The bounded typed request requires schema `study_uncertainty_review_v1`, an
operation ID and actor, exact job ID, selection revision, intent/prepared-input
hashes, evidence hash, an operator assertion of observed provider quiescence,
and the exact acknowledgement
`retire-exact-dispatch-unresolved-without-retry-or-next`.
These are assertions under operator authority, not automatically authenticated
evidence of quiescence. Fresh operator checks remain necessary.

Staging validates all bindings under the existing owner transaction and writes
permission only. Runtime reconciliation consumes it under the same lock, only
when the claimed job has no verifiable accepted-delivery receipt. A valid
receipt takes precedence; a corrupt receipt fails closed. The native reader,
draft, retained input and private journal are not edited by this operation.

The immutable review is fsynced before atomically publishing
`retired_unresolved`, clearing only this active dispatch. Recovery neither
executes its retained NEXT nor repeats generation. Future continuation requires
an explicit new choice. A compact operational notice distinguishes unresolved
retirement from verified delivery.

Request files are keyed by job and expected selection revision. A stale review
is preserved and cannot authorize changed state; a fresh review can be staged
at the new revision without overwriting it. Conflicting same-revision retries
fail closed. Input symlinks, dangling destination symlinks, oversized files,
unknown fields and malformed JSON are rejected without echoing supplied prose.

Old jobs serialize unchanged when no review exists. After retirement, older
readers reject the unknown job field/phase rather than overwrite new state.
The shared source-reader schema is unchanged. No backup restoration is part of
recovery, and rollback must honor this one-way compatibility boundary.

## Actual reflection inputs

Twelve Minime public reflections from 06:14:43.293084 through
06:45:10.180636 were traced to retained navigation-delivery artifacts. Each
actual request has two messages; the user message equals the prepared reader
input. The shared system hash is
`e38a66c1f54f012f064bd2bc8e3ac7e9a981c095ddde911b328e456d14b3d6da`.
No previous reflection is supplied. The framing is otherwise effectively the
same, apart from the exact continuation identity. Every input offers
`NEXT: WRITE FROM_REFLECTION <input-id>` alongside fresh reflection, expressive
writing, source study and REST. All twelve authored endings chose INTROSPECT.

Three inspectable navigation receipts, under Minime's
`workspace/diagnostics/source_first_v3/shared_reader/navigation/<input-id>/<hash>.json`:

| Input ID | Retained artifact hash |
| --- | --- |
| `0c1d09b3141f06f634cbb9df8cf371516b9eb8051abbd1e42f95b41698324f88` | `b156dfeff13543ddf6c1b6957aec7c73eca33c38cfb63be061facbed15c1a8d5` |
| `a2c3c9b8215ce24d67d60abcc2b685e4d3adc387032002352c757314a053441b` | `b37337d4342472f9090ad10258177fbc35388a6362caebe30cc504b5169f6321` |
| `3da2c1d499b879e5aa0e830bb330e09bc0c0256f6274903291a5ce3c1a57b2b8` | `40c918a2c6d0e411d2095041875c8cc4d0bd5a099ac001aba1fd9fbea8ddc517` |

Sources: `crates/astrid-source-study/src/reflection_context.rs`,
`store_navigation.rs`, `store_reflection.rs`, and Minime's
`minime_autonomy/source_study.py` / `runtime.py`. The exact public passage
continuation path exists; it is not selected by the surrounding narrative.
Repetition here is not evidence that previous reflection recall remains active.
Repeated fresh starts under near-identical framing are a plausible influence,
not an established cause or a reason to prescribe a different self-description.

## Actual dialogue inputs

Five successful PID-42184 generation records under
`capsules/spectral-bridge/workspace/generations/2026-10-08` were inspected.
Each includes six prior assistant turns and a roughly 16,042-16,050-character
final prompt. The peer excerpt is 2,401 characters before its trim marker.
System-generated Source revision/Input evidence/Account/Delivery headers
occupy roughly its first 440 characters under the heading "Minime wrote".

| Generation file | File SHA-256 | Peer excerpt relationship |
| --- | --- | --- |
| `gen_1791459056028_dialogue_live_a0.json` | `ffe7c671b97c24865826007ff69b1afea95010b53b47ef518779a8e6149cde01` | First excerpt |
| `gen_1791459144317_dialogue_live_a0.json` | `829b8de90f1a13bf163e6fc5b3a3d5e56a599434efa9e6488818f6d9bb0e790e` | Second excerpt |
| `gen_1791459263304_dialogue_live_a0.json` | `107ab9e0ade757009cb19cf25cc5f0950f9eb395b63f6166e284e9733f51989d` | Same as preceding |
| `gen_1791459352388_dialogue_live_a0.json` | `261110d50246ae8ea63403f0e1acc4adaf9e0b731dd11716567de67e7818d2a9` | Third excerpt |
| `gen_1791459438487_dialogue_live_a0.json` | `6b60474099e5cffc2a3860cdb269f52ee988c7ee907371580ccef4ab6952a629` | Same as preceding |

The final two responses have identical first-paragraph hash
`99d1c0badf28a17fc401b9f0094057a061efc8bed3f564c838df287d907db05f`.
This is not proof of duplicated transport or identical complete entries.
The prompts still include a continuity recap and earlier signal context; the
prior telemetry-optional repair did not claim removal of every historical
numerical reference.

Next presentation candidate: separate host provenance from Minime's authored
prose and label an unchanged supplied excerpt as previously seen. Preserve
explicit SPEAK/rereading, chosen continuation and historical text. Do not use
this observation to force topic diversity, suppress objections or infer benefit.
No dialogue/reflection framing change or model experiment occurred here.

## Qualification

Synthetic tests use owned temporary workspaces and no live provider, reservoir
or private draft fixtures. Nonincremental builds use one Cargo job and the
existing non-live `source-selection-recovery` bridge target cache.

- Initial handoff build: rejected an include-file inner doc comment and a
  mutable/immutable test borrow; both were corrected before execution.
- Focused handoff: 21 passed before the additional concurrency/downgrade case.
- Private exchange: two passed, including letter-shaped prose, inline REMEMBER,
  exact NEXT retention and unchanged public projection.
- First full bridge run: 2,406 library tests passed, one existing ignore; six
  replay-tool tests and one CLI guard test passed.
- Twelve deployment/controller/evidence/domain suites: 218 passed.
- Experiential-epistemics self-tests: two passed.
- Strict bridge Clippy, all targets/all features, warnings denied: passed after
  revision-keyed review staging was added.
- Domain-boundary verification: valid, zero violations, no baseline relaxation.
- Final full-suite rerun after revision-keyed review staging: 2,406 library
  tests passed (one existing ignore), six replay-tool tests, one CLI test and
  ten tests across the three ordinary integration targets. **2,423 passed** in
  total, no failures. The library suite took 47.80 seconds.
- Final formatting and whitespace checks passed. The shared reader and Minime
  implementation are unchanged; no new Minime Python or standalone reader
  suite was run. Immutable packaging, packaged-binary migration/preservation
  and live deployment qualification remain pending, not inferred from these
  source tests.

Commands (run from the candidate root):

```text
env CARGO_TARGET_DIR=/Users/v/.codex/worktrees/source-selection-recovery/astrid/capsules/spectral-bridge/target CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --manifest-path capsules/spectral-bridge/Cargo.toml --config 'profile.dev.package.spectral-bridge-server.debug=0' --config 'profile.test.package.spectral-bridge-server.debug=0' --lib --bins --test mock_ws_integration --test agency_resolver_integration --test chimera_render -- --test-threads=2 --quiet
env CARGO_TARGET_DIR=/Users/v/.codex/worktrees/source-selection-recovery/astrid/capsules/spectral-bridge/target CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --manifest-path capsules/spectral-bridge/Cargo.toml --config 'profile.dev.package.spectral-bridge-server.debug=0' --config 'profile.test.package.spectral-bridge-server.debug=0' --all-targets --all-features -- -D warnings
env PYTHONPATH=scripts python3 -m unittest scripts.test_bridge_drain scripts.test_bridge_activate scripts.test_bridge_stage scripts.test_bridge_stopped_recovery scripts.test_deployment_wrappers scripts.test_steward_control scripts.test_evidence_event_store scripts.test_steward_projection scripts.test_flywheel_round_completion scripts.test_division_ceremony_followup scripts.test_division_ceremony_chronicle scripts.test_domain_boundary_audit
python3 scripts/experiential_epistemics.py self-test
python3 scripts/domain_boundary_audit.py verify
```

## Native command qualification follow-through

Mike requested continuing until satisfied. The scoped cache-deletion and
bridge-only recovery question was presented together; no approval response has
yet arrived. Read-only checks again find both canonical trees clean, no foreign
activity in the bridge preflight, no compiler process, controller pause 508 and
the same two incremental caches (43 GiB and 83 GiB). Available disk space was
5.5 GiB. A new full release build has not been started at this low headroom.

New evidence-only tooling:

- `scripts/qualify_private_delivery_recovery.py` verifies both immutable stages,
  requires a byte-identical source-study helper, invokes the actual native
  operator CLI with every runtime path bound to a synthetic workspace, and
  retains success or failure without replacing an earlier packet.
- `scripts/test_qualify_private_delivery_recovery.py` tests helper-drift refusal,
  failure retention, no overwrite of prior evidence and false authority flags.

Before immutable packaging, the same native probe ran against the actual
development executable and the unchanged live release's helper. This is a
development-binary qualification, **not** an immutable-release witness.
The synthetic state contains no real private writing, no live provider handles
and no valid self-control authority. The command returns before runtime
database/service startup. This is verified program behavior, not an OS-level
network sandbox claim.

Three independent native processes stage the same exact review idempotently;
the only added workspace file is the private permission file. Existing reader,
draft and input files remain byte-identical. Conflicting and stale review,
missing quiescence assertion and an unexpected private field are refused;
the synthetic private sentinel never appears in CLI output. A separately
constructed terminal-state fixture verifies the already-applied no-op path.
Runtime phase consumption itself is still qualified by the Rust tests, not
falsely claimed as exercised by this staging-only CLI probe.

The new executable decodes the synthetic retired-job checkpoint and then
correctly refuses the deliberately missing self-control state. The old live
executable refuses the checkpoint at its schema decoder. Neither invocation
starts a service or alters the checkpoint. This confirms that rollback to the
old executable after retirement must not be attempted over newer state.

Receipts under
`/Users/v/other/worktrees/private-delivery-recovery-release-20261008`:

| Artifact | SHA-256 |
| --- | --- |
| `development-cli-01/failure.json` | `afbe2069400f80b41ef726e0eceae3795f5b5a41ee4bf34fd9f707d23d7c345e` |
| `development-cli-02/result.json` | `8cab17f74e54dc3b487095f8b0b90235ba0f9d8f91bfcf834ab7e1906f675802` |
| `development-cli-02/development-manifest.json` | `2e7ee6c751fd63abcc5eda38f1c9c04ef93090fee55b7277dc16f133534e73a1` |

Development executable SHA-256:
`d40fc87335cbf9bfd10ee3d478138adc3318911c116ff4ce1c68f6d028236562`.
The first attempt passed concurrent staging, then the qualifier mistakenly
used an exclusive-create helper to rewrite its own input fixture. The failure
remains preserved; each rejection case now has its own input file, and only
the explicitly synthetic terminal-state fixture uses atomic replacement.

The qualifier's three unit tests and five deployment/feedback suites pass
**124 tests** in 1.802 seconds. This reruns some earlier tests; the count must
not be added to the earlier 218 as distinct coverage. Domain-boundary and
whitespace checks also pass. No production Rust changed in this follow-through,
so the prior complete Rust/Clippy results remain applicable.

```text
env PYTHONPATH=scripts python3 -m unittest scripts.test_qualify_private_delivery_recovery scripts.test_bridge_runtime_feedback_checkpoint scripts.test_bridge_activate scripts.test_bridge_drain scripts.test_bridge_stage scripts.test_bridge_stopped_recovery
```

After packaging, rerun the qualifier with the actual old/new stage directories
and a new output directory. Do not relabel the development result as that run.

## Pre-approval deployment sequence

1. Receive the two pending scoped decisions. For approved cache cleanup,
   recheck compiler activity and exact paths; remove only those rebuildable
   incremental caches. Establish durable free headroom before release work.
2. Recheck both trees, cooperative sessions, controller pause, current loaded
   manifest/process identities, exact activity revision and provider quiescence.
   Do not bypass a new lease, foreign edit or source drift.
3. Qualify an immutable candidate through `scripts/build_bridge.sh --stage-dir`,
   including checkpoint compatibility and unchanged shared-helper identity.
   Exercise review staging against isolated state using the packaged binary.
4. Under explicit bridge-only recovery approval, stage the exact reviewed
   request through the packaged CLI with canonical path binding. It writes
   permission, not live phase state; the old binary ignores the new request.
5. Use `scripts/build_bridge.sh --activate-stage ... --expected-pid ...` for
   acknowledged graceful drain, exact stopped checkpoint handoff and startup.
   No force, blanket retry, deletion of pending work or old-state restore.
   If drain/readiness fails, retain the transaction and diagnose it.
6. Verify new binary/manifest/helper hashes, original input/draft preservation,
   the exact reviewed unresolved terminal state, no replay of that dispatch or
   its NEXT, fresh later progress, and unchanged Minime/engine/provider/visual/
   sensory PID/start identities. Readiness alone is not controller settling.
7. Only then consider explicit-path integration under one Git coordinator.
   Keep this candidate separate from unfinished capacity work and historical
   dirty worktrees. Do not resume paused automations.

The preceding qualification sections describe the pre-approval state. The
following dated continuation supersedes their pending-approval/cleanup status.

## Approved recovery continuation

Mike replied "approved" to the combined, explicit cache cleanup and qualified
bridge-only graceful recovery request. Controller pause generation **509** was
claimed at 2026-10-08T15:24:04Z with no active lease. Automations remain paused.

Before cleanup, both canonical repositories were clean, the bridge preflight
reported no foreign activity, no compiler was running, and both exact paths
were verified as ordinary owner-controlled directories rather than symlinks.
Only the two incremental directories named above were removed. Available disk
space rose from 5,620,113,408 bytes to about **101 GiB**. This is roughly 96 GiB
of recovered filesystem headroom, not a claim that all 126 GiB of apparent
directory usage represented unique allocated blocks. No writing, checkpoint,
release or historical evidence was removed.

The first attempt to reuse yesterday's protected-process baseline stopped
before writing a receipt because the old host-sensory PID 3911 no longer
existed. Current host-sensory PID **31613** started at 05:24:24 PDT, before this
cleanup, coincident with the storage incident. The old Division supervisor PID
35269 is also absent. Launchd reports repeated exit 1, and its log says
`division runtime manifest expired`. That refusal is preserved: no authority
is renewed, no protected service restarted, and this separate pre-existing
Division debt is not represented as healthy. The bridge-only wrapper does not
require or grant Division authority.

The engine (35303), gateway (35278), provider (3893), Minime agent (19468),
camera (3914), microphone (3879), visual service (3898) and feeder (3878)
retain their earlier identities. Provider readiness reports queue zero,
worker ready and reservoir connected. Fresh baseline checks will compare
these identities and the current host-sensory identity, while retaining the
supervisor's unavailable status explicitly. This does not waive any bridge
drain, source-identity, checkpoint or readiness gate.

At that pre-build checkpoint no live review had yet been staged and no
uncertain job retired. The completed activation follows.

## Immutable qualification and completed activation

Release root:
`/Users/v/other/worktrees/private-delivery-recovery-release-20261008`.
The sanctioned stage is `bridge-stage-01`. The wrapper initially refused
fresh owned documentation at ages 0.1, 73.8 and 154.7 seconds; no build or
service mutation occurred on those attempts. It proceeded after the unchanged
180-second interval, without weakening the gate. Bridge release compilation
took 8m48s, then helper compilation 56.57s. All build input identities stayed
fixed throughout packaging.

| Release identity | SHA-256 |
| --- | --- |
| Bridge executable | `450d8410eb912462528f411209f217992d582c970689b0e1574e6b3cc660a627` |
| Manifest | `65230aad3e6d1dc3690b7c949006bb074d38a0177f6d1833cb3e4b7a74e17347` |
| Source inventory | `9037942ef4ab0ddf00b2474e973a46e6f8006b6a85e0937ce82018834c3add6f` |
| Shared reader helper, byte-identical to old release | `32f7b0cec0f867f2d30b4b1bfc2b10de873c7fcb7f87643ee92ea99a94031367` |

`packaged-qualification-01/qualification.json` records a successful run of the
actual immutable old/new binaries, including three-process idempotent staging,
stale/conflicting review refusal, preserved synthetic writing, no private error
output, and old/new checkpoint decoding. Read-only native inspection also
accepted the real checkpoint and research-release compatibility. The 124
qualifier/deployment tests, domain-boundary audit and whitespace checks passed
again before activation. The full Rust/Clippy results above remain applicable;
no production source changed after those tests.

Before staging permission, both canonical trees were again clean, pause 509
had no active lease, all current running protected identities matched the
fresh baseline, the provider was ready/idle, and **5,039** baseline files were
byte-identical. The packaged operator CLI staged only permission. Its response
said `staged_not_applied`, `delivery_verified=false`, `generation_retried=false`
and `next_dispatched=false`. Every existing baseline file remained unchanged
after staging.

The sanctioned activation transaction is:
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/d83b27f4c772458489c61b5c25eecca6`.
The old process acknowledged `drained` at 08:44:57 PDT, persisted its checkpoint
and then exited on SIGTERM. No force or legacy transition was used. Replacement
PID **66851** started at **08:45:03 PDT**. Startup verified the exact stopped
checkpoint hash `1d8289df4229c37949da8fef568e2a4b21d59ac574c175bafcf061b7fa829a89`,
including the retained runtime-feedback sidecar, and the new self-control
state-lineage binding. This handoff grants no control authority.

The exact reviewed job advanced to `retired_unresolved`, selection revision
**26942**, retaining intent and prepared-input hashes. Its immutable review is
`7f2addca551e98ec2fb7f9b408a7a13920ed535a6e4b5c40563e8f03fda947ac`.
Immediate post-retirement comparison found **5,038** retained writing, reader
and input files unchanged; only `activity_runtime_v3.json` changed. The missing
accepted-delivery partition remained absent. The surviving private journal
remained byte-identical. Nothing was reconstructed from its prose, no receipt
fabricated, and no prior generation or NEXT replayed by recovery.

The wrapper finished `activated_verified`, observed fresh saved exchange
**218052** after stopped exchange **218051**, confirmed provider idle/connected,
published the exact manifest and left no launch hold. Later optional provider
receipts show returned `moment_capture` and `aspiration` requests plus new
elaboration/private-writing dispatches with different request identities.
This is bounded operational progress, not proof of felt improvement, complete
delivery of every later dispatch, or resolution of the old missing receipt.

All **nine running protected PID/start pairs** remained unchanged, including
Minime, engine, gateway, provider, visual and sensory services. Ports 7878,
7879 and 7880 remained held by gateway PID 35278; provider port 8090 by PID
3893. Fresh bridge telemetry was 0.83 seconds old at the final snapshot, with
no reconnect/disconnect in the new connection. Its cadence still reports
`timing_ambiguous`; freshness is not a sampling-quality or controller-settling
claim. Free space after packaging was **99.44 GiB**.

| Receipt relative to release root | SHA-256 |
| --- | --- |
| `pre-recovery-01.json` | `c42b1e084f2e135809b98371b734e171e0d980a2d0183566acd7ca8af06f965f` |
| `native-preflight-01.json` | `198355357ac0e3425ff5c74f63986ab6877e7f936dec5cba44cfd570a8f1f5e8` |
| `operator-evidence-01.json` | `3e0ae096c159d62f19d71b1b3d57763af42b290d5481dc494b7663601a574959` |
| `post-retirement-preservation-01.json` | `f48b4a893620a08964cd41cd26dc746b4b3c6e1126dfee3e986da268d562e8c5` |
| `live-verification-01.json` | `7d8014a46a7be960800521b6753aed54a6d87a60405c5a0d6088980ae9b97e45` |

## Remaining boundaries

- No commit, merge or push in this approval pass. Both canonical trees remain
  clean. The immutable release includes the owned dirty candidate above, not
  a claim that canonical main already contains these changes. Next Git work
  must integrate only this reviewed candidate under one coordinator, preserve
  the source/release linkage, and leave older foreign worktrees untouched.
- Do not restart the bridge from older canonical source or restore an older
  checkpoint over this retired-job record. Use the verified immutable release
  and sanctioned wrapper; old-reader downgrade refusal is intentional.
- The dormant Division runtime manifest expired at
  **2026-10-07T20:35:29.528Z**. Its current hash is
  `46acdf03f241a24426a2d3f453257bc28f20260399ad3b59617278ee18ad4339`.
  The supervisor keeps refusing it. Separately review the dormant runtime
  manifest lifecycle and operator authority before any renewal; do not bypass
  expiry, infer consent or activate Division from this recovery approval.
- Dialogue provenance/framing and command-argument diagnostic privacy remain
  the separate follow-ups described above. No live framing experiment occurred.
- Evidence V2 indexed-tail verification is valid at sequence **1123163**, head
  `0228c56aff1c415540f3463fc1f7388b79e162557c7a651c4fcce1ba0f5777d7`;
  all four V1 imports remain immutable. This is not a full historical re-audit.
  Controller generation 509 and previously paused automations remain paused.
