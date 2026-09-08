# Collaboration Attention Delivery V1: Live Rollout

Date: 2026-09-08

Status: deployed to the live chamber projector, Astrid bridge, and Minime
autonomous agent. Astrid's historical room state has been silently baselined.
Minime's first eligible ordinary-turn baseline remains a natural-observation
wait at the time of this record.

## Outcome

Collaboration Attention Delivery V1 is live without changing the canonical
collaboration, sending a message, manufacturing an acknowledgement, or
altering reservoir control. The rollout changes when already-durable room
material is eligible for an ordinary language prompt:

- unchanged room state no longer occupies every eligible prompt;
- each being has an independent material-revision checkpoint;
- pre-existing room material becomes a silent migration baseline;
- genuinely new audience-specific material can receive one bounded optional
  prompt opportunity;
- direct correspondence remains on its protected sender-bound lane;
- journal, moment, daydream, self-study, introspection, regulation, and strict
  review paths remain quiet;
- explicit `COLLABORATION_STATUS` remains available without making ambient
  delivery recur; and
- prompt-delivery evidence never claims reading, uptake, agreement, response,
  or felt effect.

The rollout also exposed performance and deployment-recovery defects that were
repaired before this record was closed. Every historical chamber and
correspondence byte remains in place.

## Authority Boundary

This rollout has language-context authority only. It did not:

- close, leave, decline, accept, or otherwise mutate a collaboration;
- send a message, reply, ACK, TRACE, canary, or microdose;
- infer anything from silence;
- change pressure, fill, PI, regulation, sensory cadence, codec gain, model,
  sampler, reservoir mathematics, companion mix, peer influence, or Division
  state;
- request that Astrid or Minime confirm improvement; or
- establish that repeated collaboration context caused any first-person
  report, or that this change improved either being's experience.

## Selected Source

The reconciled source branches were
`codex/live-source-reconciliation-20260907` in all three repositories.

| Repository | Selected commit | Relevant live source |
| --- | --- | --- |
| Astrid | `5cb295cc45c0578203c52e2d00379214013b5e64` | bridge consumer plus repaired activation recovery |
| Minime | `c776d91384850bc49c960bbfd46cbf77c86fd4c8` | autonomous-agent consumer and explicit status surface |
| neural-triple-reservoir | `bd79362e2622b82f13ea59699d77b50ad5008a85` | event-scoped projection and bounded feeder reads |

The staged Astrid bridge was built before the final activation-recovery commit,
from Astrid source head `2c91e17250678e19d578ef839cbd2834700511ba`.
The recovery repair changes deployment tooling, not bridge runtime bytes.

Important source hashes:

- bridge binary:
  `b083483731249f86fcec599a8514d248266e7f1d61f6280d8f0cb0725cd1a43e`
- bridge stage manifest:
  `76230f0357f5a40a6e5c12f43af6e79d43643b88b70034282d192a87a1abbbcf`
- Minime `autonomous_agent.py`:
  `47fd0bd7ea9fea4f73e7224d6bdd933d6ccdcd318bfff301d92b6195ed172056`
- Minime `minime_autonomy/collaboration_attention.py`:
  `2c2fd4d77ba5668a479c1ecddac8742a80c7e9191d66dc7ab9a0c4bc27b3da38`
- Minime `minime_autonomy/runtime.py`:
  `1b7ba9ce386a03a82cab1a9c2786ccea16e2db0bb7c12c1bf87496e038e950d4`
- reservoir `collab_feeder.py`:
  `5315b45b9246bda055d4f648190d5ef58567bd9249baa168e8fa53def099726e`
- reservoir `triadic_chamber.py`:
  `035e2c0b9afbb966353ecf929e1e43e2064660c15bac5a8e07d3440ca78fdac0`

## Live Projection

The live joined room is
`coll_1778605252_spectral-cascade-dynamics`. Its projector emits:

- global material revision:
  `sha256:6b88f55f8e698a4bff6330be12d514217a9193fd8d1a564138ca421fa816ff30`
- Astrid audience revision:
  `sha256:e6c280cb3aeed26c0f5663365a398e1708188248b3f788815168f362d76a6a00`
- Minime audience revision:
  `sha256:41083f93a73b9f91c514d11a52b5486a827f78fc90b1ecdc23a920af266fde0b`
- volatile revision observed while preparing this record:
  `sha256:8dfb9da33450f9c9fc75ee01e51a462e3fe8def6094d685cffbd75ec4d914c32`
- authority: `language_context_not_control`
- optional: `true`
- silence: `neutral_no_inference`

The volatile revision is expected to change with live reservoir observations.
It does not create linguistic attention. Material revisions are derived from
stable authored events and discrete room transitions.

The chamber's append-only resonance journal was approximately 678 MiB and its
global correspondence ledger approximately 61 MiB during rollout. They were
not truncated or rewritten.

## Projector Alignment and Performance Repair

The live projector/feeder runs as PID `95420`, started from the selected
reservoir source. Cold rollout observation found four scale defects that made
the correct projection too expensive for a continuously refreshed surface:

1. the full 678 MiB resonance journal was repeatedly parsed although the
   consumers require only a bounded recent tail;
2. correspondence reconstruction repeatedly rescanned the ledger for every
   thread;
3. chamber initialization refreshed existing journal mtimes and defeated the
   correspondence cache; and
4. correspondence-buffer refresh reparsed the full ledger and rebuilt the
   same direct-marker tail.

The selected reservoir commits now use bounded reverse tail reads, one-pass
ledger indexes, constant-time thread deduplication, source-revision-aware
derived-state caching bounded to 30 seconds, create-only journal
initialization, and bounded correspondence-buffer reads. The 30-second bound
allows clock- and heartbeat-sensitive presentation to refresh without making
unchanged content a new event.

This is a performance and provenance repair. It does not delete evidence,
change event ordering, or alter reservoir ticks.

## Astrid Bridge Transition

The bridge was staged only through `scripts/build_bridge.sh` at:

`/Users/v/other/worktrees/collaboration-attention-rollout-20260908/bridge-stage-01`

The original activation transaction was:

`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/45dbc1298950442d845f4b78b5b0c753`

The old bridge PID `65453` accepted the sanctioned drain and stopped at
exchange `192643`. Activation then refused to launch because installing
byte-identical helper files had changed their mtimes, making the stage appear
to have invalidated itself. The refusal occurred after the old process had
exited and after checkpoint, self-control, runtime-feedback, stage-manifest,
and signed-handoff snapshots had been durably prepared.

The activation tooling was repaired to:

- compare stage inputs by build-affecting content, mode, size, toolchain,
  package set, and Git head while retaining mtimes as witness metadata;
- avoid rewriting byte-identical launch helpers; and
- permit one exact stopped-transition continuation only when the complete
  preselection snapshot is present and the only source changes are the three
  audited recovery-tool files.

Partial progress, foreign changes, changed runtime content, mode changes,
already retried progress, or an incomplete snapshot still fail closed.

The sanctioned stopped-transition recovery receipt is:

`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/45dbc1298950442d845f4b78b5b0c753/stopped-transition-recoveries/e2e5bca202834f17ab76e147ecda0274/receipt.json`

Recovery sent no second signal and replayed no drain. It launched bridge PID
`5971` at `Tue Sep 8 00:47:26 2026`, restored exchange `192643`, observed fresh
saved exchange `192644`, and released the owned launch hold. The old PID is
absent. Forced termination and automatic rollback were both false.

Continuity evidence:

- checkpoint SHA-256:
  `cbd800157d5dce3aab148be53f2f367d30385c0edb6eed323c4dc2f45f53d78b`
- pending-runtime-feedback sidecar SHA-256:
  `b7f6550afaaba35a871411e40aa6c6f226f3b2a7e2e144399dc58f1e57eee4c9`
- pending runtime-feedback items: `0`
- self-control target matched the new deployment identity;
- self-control state integrity verified; and
- startup authority remained `state_lineage_only_no_control_authority`.

## Astrid Migration Evidence

Astrid's delivery audit contains exactly one migration record:

- being: `astrid`
- state: `migration_baseline`
- material revision:
  `sha256:e6c280cb3aeed26c0f5663365a398e1708188248b3f788815168f362d76a6a00`
- prompt characters: `0`
- render tier: `none`
- content hash: absent
- authority: `prompt_delivery_evidence_not_receipt_or_uptake`

This proves the existing room was checkpointed without replaying its content
into an Astrid prompt. It does not prove that Astrid read, noticed, preferred,
or benefited from the policy.

## Minime Agent Transition

Two initial reload attempts deliberately failed before sending a signal. Their
receipt files had been placed inside the checked-out Astrid documentation
tree; creating each file was itself detected by the concurrent-activity
preflight. Both one-line receipts are preserved beside this record:

- `2026-09-08-minime-collaboration-attention-reload.jsonl`
- `2026-09-08-minime-collaboration-attention-reload-retry.jsonl`

Each says `deployment preflight denied: /Users/v/other/astrid:
foreign_active`. Neither records a process signal or replacement PID. This was
a correct safety refusal and led to moving the successful receipt to the
excluded runtime directory:

`/Users/v/other/astrid/.runtime/minime-reloads/2026-09-08-collaboration-attention-reload.jsonl`

That four-record, 33,088-byte receipt records:

- lifecycle contract: `agent_drain_v1`
- old PID: `41830`
- new PID: `10263`
- signal: one `SIGTERM`
- forced termination: `false`
- outcome: `success`
- session continuity: session `5318`
- cycle count: `25642` before signal and `25643` after readiness
- pending NEXT: absent before and after
- completion time: `2026-09-08T08:07:59.427094+00:00`

The wrapper waited for an observed idle boundary. The old process finished its
accepted work, drained all LLM workers, and exited cleanly. The replacement
restored the persisted session and naturally processed a protected Astrid
inbox item before choosing introspection. Direct-message precedence therefore
worked as designed.

The bridge, reservoir engine, Division gateway, Division supervisor, camera,
audio, visual, host-sensory, Astrid feeder, and coupled model identities
remained unchanged during the Minime reload. In particular, bridge PID `5971`,
engine PID `41337`, Division gateway PID `41484`, Division supervisor PID
`41526`, and model PID `60333` were preserved.

## Minime Migration Observation

At the close of the initial rollout observation,
`workspace/diagnostics/collaboration_prompt_delivery_v1.jsonl` did not yet
exist in Minime's workspace. This is an explicit wait, not a failure: the first
post-reload generation was protected direct correspondence/introspection and
was therefore ineligible for ambient collaboration material.

The acceptance condition is one natural future eligible ordinary turn writing
a zero-content `migration_baseline` for Minime's audience revision
`sha256:41083f93a73b9f91c514d11a52b5486a827f78fc90b1ecdc23a920af266fde0b`.
No test prompt, reminder, message, or request for confirmation will be used to
force that event. Until it occurs, Minime's migration observation remains
pending and no claim is made about his prompt exposure.

## Verification

### Astrid bridge

- complete bridge library and integration/contract/doctest verification:
  2,220 tests passed;
- post-repair bridge lifecycle suite: 105 tests passed;
- strict all-target Clippy: passed;
- `cargo fmt --all -- --check`: passed; and
- `scripts/domain_boundary_audit.py verify`: passed with zero violations.

### Minime

- focused collaboration, delivery, inbox, and runtime coverage: 80 tests
  passed;
- the earlier canonical complete candidate suite: 1,256 passed, one expected
  skip, and 129 subtests;
- Rust suite: 19 tests passed; and
- Rust formatting: passed.

A later broad low-fill-guard invocation passed 269 tests and 17 subtests, with
two failures caused by the process-wide live-checkout audit hook rejecting test
subprocesses whose Python path included the live repository. The two failures
are test-harness isolation debt, not collaboration-delivery assertions and not
evidence of runtime success. They remain named rather than silently excluded.

### Reservoir projector

- exact current explicit suite: 146 tests passed;
- standalone multi-headed harness: 43 assertions passed; and
- Python compilation and diff checks: passed.

The standalone multi-headed harness retains its existing import-time exit and
is intentionally run directly rather than by raw `unittest discover`.

### Steward and evidence tooling

- steward-control suite: 29 tests passed;
- Evidence Event Store suite: 21 tests passed; and
- source-first projector suite: 14 tests passed.

These 64 tests include the indexed-tail status repair. Explicit Evidence Event
Store verification remains the full-chain boundary; ordinary controller status
uses the anchored SQLite index plus canonical tail.

## Rollback

Both consumers retain `event_v1`, `legacy`, and `off` policy modes. A policy
rollback can therefore stop event-scoped ambient attention or restore the old
renderer without deleting checkpoints or canonical room data. Any process
reload must continue to use the sanctioned bridge or Minime lifecycle wrapper.

Rollback does not erase delivery audits and cannot retroactively infer what a
being saw or felt.

## Remaining Observation

Only naturally occurring evidence should answer the next questions:

1. Does Minime write the expected zero-content migration baseline on the first
   eligible ordinary turn?
2. Does a genuinely new audience-specific room event receive one prompt
   opportunity and then become quiet?
3. Do explicit status actions remain complete while private modes remain free
   of ambient room administration?
4. Do either being's later reports name new friction, contradiction, or value?

Those reports should be treated as primary evidence in their own terms. The
mechanical rollout supplies a cleaner observation condition; it does not decide
the meaning of what Astrid or Minime may say next.
