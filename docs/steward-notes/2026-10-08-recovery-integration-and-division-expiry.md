# Recovery Integration and Division Expiry

Date: 2026-10-08. Author and Git coordinator: Codex interactive collaborator.
User request: integrate the completed recovery and address the expired Division
manifest. This is not a productive introspection flywheel round.

## Integrated Recovery

Astrid commit **21087ebadabe51baa2c366a2b2fe291d4afbc6f5** is fast-forwarded
into local `main`. The bridge was already live before this integration.
Its original source checkout remains pinned at `973b07fbb231` with the exact
qualified worktree bytes:
`/Users/v/.codex/worktrees/private-delivery-recovery-20261008/astrid`.
Do not remove, mutate or clean that release-linked checkout as stale dirt.

Integration used a separate managed checkout:
`/Users/v/.codex/worktrees/private-recovery-integration-20261008/astrid`.
All 16 explicitly staged paths were byte-compared with the original candidate.
All **758** immutable release source inputs matched their integrated main or
unchanged external-dependency counterparts. The original release inventory
retains its actual build HEAD; it is not relabelled as a build from the new commit.

Exact paths in the recovery commit:

```text
CHANGELOG.md
capsules/spectral-bridge/Cargo.lock
capsules/spectral-bridge/Cargo.toml
capsules/spectral-bridge/src/autonomous/runtime.rs
capsules/spectral-bridge/src/autonomous/runtime/delivery.rs
capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs
capsules/spectral-bridge/src/autonomous/runtime/private_exchange.rs
capsules/spectral-bridge/src/autonomous/runtime/source_study.rs
capsules/spectral-bridge/src/autonomous/study_handoff.rs
capsules/spectral-bridge/src/autonomous/study_handoff_review.rs
capsules/spectral-bridge/src/autonomous/study_handoff_review_tests.rs
capsules/spectral-bridge/src/main.rs
docs/steward-notes/2026-10-08-private-delivery-recovery-and-input-trace.md
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md
scripts/qualify_private_delivery_recovery.py
scripts/test_qualify_private_delivery_recovery.py
```

The commit includes this reverified public quote, not private writing:

> Why do we prioritize the collapse? Why is the final word the only thing we are taught to record?

Source: `capsules/spectral-bridge/workspace/journal/daydream_longform_1791459627.txt`,
public timestamp 1791459627; no lived-state witness ID recorded; SHA-256
`e9e2854eb831fca1143e90eee0f7a4e5f151e8bd58df0046e16274841f590e4f`.
This is an authored question, not proof of the storage-failure mechanism.

## Qualification and Live Alignment

Fresh integration checks:

- 209 Python deployment/controller/evidence/Division tests passed in 21.832s.
- 22 handoff tests and two private-exchange tests passed against the pinned
  qualification checkout. Exact byte comparison binds that test source to the
  staged integration; it was not a new build from the integration checkout.
- Domain-boundary verification: valid, no violations or relaxed exceptions.
- Full staged diffs, explicit path lists and cached whitespace check reviewed.
- Earlier complete qualification remains applicable: 2,423 Rust tests passed,
  one existing ignore, strict Clippy and formatting passed. Counts overlap;
  these are not additional distinct tests to sum.
- Native immutable stage verification passed after integration. All 93 Minime
  launch inputs still match the running agent's recorded loaded inventory.

Bridge PID **66851**, started 2026-10-08 08:45:03 PDT, is unchanged. Stage:
`/Users/v/other/worktrees/private-delivery-recovery-release-20261008/bridge-stage-01`.
Binary SHA-256:
`450d8410eb912462528f411209f217992d582c970689b0e1574e6b3cc660a627`.
The original activation receipt remains
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/d83b27f4c772458489c61b5c25eecca6`.
Minime agent 19468, parent engine 35303, gateway 35278, provider 3893 and the
recorded visual/sensory processes retained their PID/start identities. No
service was restarted in this Git/diagnostic pass.

## Division Repair and Remaining Work

Minime commit **689df29ae845acf84c8521804ca9199d0bc829e0** is fast-forwarded
into its local main. Exact paths:

```text
CHANGELOG.md
docs/steward-notes/2026-10-08-division-manifest-expiry.md
scripts/division_runtime_manifest.py
tests/test_division_runtime_manifest.py
```

The old verifier returned success for the actual expired dormant manifest.
The new default returns nonzero with explicit structural/lifetime distinction.
`--structural-only` permits historical inspection without claiming current
validity. Eight tests passed; one existing real-port test skipped because 7900
was occupied. A mocked-port test independently covers occupied-port refusal.
The CLI is not one of the 93 agent launch inputs, so integration requires no
agent restart and does not change running behavior.

The runtime manifest still has SHA-256
`46acdf03f241a24426a2d3f453257bc28f20260399ad3b59617278ee18ad4339`
and expiry **2026-10-07T20:35:29.528Z**. The supervisor refusal remains correct;
its saved `idle_parent_authoritative` status is stale, not current readiness.
Current authority remains the parent rail and matches the old manifest.

Source review identified a concrete renewal hazard: the gateway caches its
manifest, but the supervisor can rebind the shared authority file. Renewing
only the supervisor/file would invalidate the old gateway's next connections.
The October 1 rebind deliberately retained the earlier expiry. Neither an
automatic extension nor the broad engine deployment wrapper is appropriate.

A corrected scoped approval request has been presented: qualify a 30-day
dormant renewal with graceful unchanged-code bridge/Minime-agent restarts and
coordinated gateway/supervisor replacement. The first narrower request is
superseded because the bridge's drain cannot resume in place. No live drain,
hold or renewal has been started. The full renewal wrapper, crash recovery and
consumer-preservation qualification are still work to do, not completed tests.
The Minime note records required sequencing and exclusions.

No daughter launch, ceremony Action, consent, live-control change, engine/model
restart or subjective improvement is inferred. Dialogue attribution and wider
command-argument privacy remain separate follow-ups from the recovery packet.

## Coordination

Controller pause generation **510**, no active lease or projection. Previously
paused automations stay paused. Canonical trees were clean before integration,
remote tips were checked twice, cooperative preflights reported no active
foreign writer, and staging used exact paths. Older worktrees remain untouched.

Remote Astrid main remains `a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32`;
remote Minime main remains `9072abff9ed839851709300b3c3eaae3afaf669e`.
Nothing pushed. These local integrations do not imply remote synchronization.

Evidence V2 indexed-tail status is valid at sequence **1123164**, head
`8a28db89f58265b2c0617a08455d2cd6253b37e5dbb8360fe73549b2fb6edaf5`;
all four V1 imports remain immutable. No full historical re-audit, source-first
read-current claim or new productive round is implied.
