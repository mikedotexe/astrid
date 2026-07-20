# Incremental Projection Runtime V3 Rollout

## Decision

V3 is implemented and validated on
`codex/incremental-projection-runtime-v3`, based on `b904d60732`. It is ready
for review, but it is not authorized for the live controller yet. The dirty
live worktree, Astrid `main`, and both paused automations were not changed.
No runtime restart is required because this tranche changes paused
evidence tooling only.

## Invariants

- Evidence Store V2 `events.jsonl` remains the canonical, append-only,
  hash-chained record.
- `EvidenceReadIndexV1`, generation journals, projector cursors, and
  felt-contract state are owner-only derived artifacts. They grant no
  authority and may be rebuilt.
- A controlled projection session performs one full V2 verification, anchors
  to that head, and validates indexed tails under the existing append lock.
- A step is reused only when its declared stream watermarks, source hashes,
  dependency outputs, command/config identity, projector version, output
  hashes, JSON, counters, and authority scan all pass.
- A failed or incompatible resume journal is retained. It cannot replace the
  prior successful generation.
- Pause requests receive one cooperative `SIGINT`; lease renewal continues
  while the child drains. V3 never force-kills or preempts.
- Every generated artifact remains `evidence_only` or `approval_pending`.
  Nothing in V3 can approve, deploy, dispatch, alter live controls, or become
  an authority source.

## Acceptance Metrics

The acceptance copy contained more than 351,000 canonical V2 events and used
the complete nine-step source-first profile.

| Fixture | Result | Steps | Controller duration | Observed wall | Peak RSS | Gate |
|---|---:|---:|---:|---:|---:|---:|
| No input, cycle 1 | passed | 0 executed, 9 reused | 2.834 s | 20.974 s | 445,693,952 B (425 MiB) | <45 s, <512 MiB |
| No input, cycle 2 | passed | 0 executed, 9 reused | 2.748 s | 21.410 s | 447,758,336 B (427 MiB) | <45 s, <512 MiB |
| 20 new canonical reports | passed | 6 executed, 3 reused | 37.350 s | 55.707 s | 857,849,856 B (818 MiB) | <3 min, <1.5 GiB |
| Full rebuild | passed | 9 executed, 0 reused | 49.777 s | 69.075 s | 1,682,096,128 B (1.57 GiB) | <12 min, <3 GiB |

Both no-input generation manifests had identical before/after V2 heads and
appended no claim-family or felt-contract events. The 20-report fixture
advanced V2 by exactly 21 events: 20 addressing events and one bounded
generation receipt. The full rebuild preserved deterministic outputs and
required exact incremental/reference parity.

## Migration Evidence

The claim-family baseline contains 5,699 claims in 5,696 families, with every
claim assigned exactly once. The logical migration receipt preserves 93,432
historical family events and 93,483 historical membership events, including
87,736 duplicate family and 87,784 duplicate membership restatements. It
records `history_rewritten=false`; no event was deleted or rewritten. An
unchanged incremental pass generated zero events and consumed zero new family
events.

The felt-contract state contains 5,699 claims in 5,696 stable contracts,
73,090 nodes, 67,409 edges, and 84,510 graph events. All identity, membership,
parent, closure, silence, evidence, and authority counter checks are
consistent. The full and incremental projections matched exactly, including:

- `contracts.jsonl`:
  `460001fe84e6a8e55fd273fc5ebda071c9aae5ae5512da7e9ab0c65a449b549d`
- `report.md`:
  `af1917ec136da122d8952b2a770137fb478a34e3987a3023b3d8f7bf608928c3`

## Final Copied-Store Receipt

After acceptance, the isolated controller was durably paused at pause
generation 3 with no active lease or projection. Final V2 verification:

- global sequence: `351156`
- head SHA-256:
  `50f018a8817b4ecd71e289a4c8f289778728c14743d2f77dafff713d61f4199e`
- addressing: 39,297
- claim families: 211,973
- Corridor V1/V2: 3 / 112
- felt contracts: 84,510
- model QoS: 9,988
- Sandbox: 2,019
- Signal Spine: 2,912
- steward control: 342

All four immutable V1 sources matched their activation hashes:

- addressing:
  `4a69dc092c1bcad8e157936f11f7798d67a883869bcfe56816fdf1be5ec78571`
- Sandbox:
  `eac68fe839042c981756c2ec3b5c64f5a2633fdb75847a14fbd98c8f64ec4ebb`
- Corridor V1:
  `e190046e1b583d5b7b4a624ab50314fafbb6e0d751d9605f1ce9e85f148e01e4`
- Corridor V2:
  `e0ddb5e715d9a20cc709402fb1eda4712a1de001ea23730c70a349096468ccd5`

The read index matched canonical bytes, head, event count, and every stream
counter, had mode `0600`, and reported `authority_source=false`.

## Validation

- 54 focused Python integration tests passed.
- All 13 steward, event-store, authority, experiential projector, Corridor,
  addressing, Sandbox, recent-signal, and proactive-scan self-test entry
  points passed.
- Full Astrid Rust workspace tests and doctests passed with the QuickJS
  end-to-end fixture enabled. The locally generated test kernel did not alter
  the tracked kernel hash.
- `cargo clippy --workspace --all-features -- -D warnings` passed.
- `cargo fmt --all -- --check` passed.
- Python compile checks, shell syntax, portability scan, and `git diff
  --check` passed.
- Architecture-health review led to extracting felt-contract incremental
  orchestration from the public CLI. The CLI is 691 lines and its reusable
  runtime module is 428; all new generic V3 modules are below 1,000 lines.
  It also split the coordinator's former 410-line transaction into bounded
  preparation, command, step, publication, and failure helpers. Two deliberate
  long-function signals remain reviewer-visible: the 238-line source-first
  profile is a centralized declarative registry, and the 249-line felt
  projection transaction keeps append, checkpoint, and full-parity decisions
  in one auditable ownership boundary.

## Upstream Boundary

The cooperative lease guard, process runner, V3 input/checkpoint identity,
bounded receipts, generation journal, and rebuildable index interfaces are
upstream candidates. Astrid's source-first graph and experiential projectors
remain fork-specific. No upstream issue or pull request was opened.

## Resume Gate

Resume decision: **hold**. Review this branch and its performance receipts
first. Only an explicit later instruction may integrate V3 into the live
worktree and resume catch-up. Review does not grant deployment, approval, or
live-control authority.
