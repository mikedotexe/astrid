# Activity foundation: main/live source reconciliation

September 6, 2026, Pacific time. Owner: Codex `/root`, task
`01a07499-9e61-7fe3-b9c4-8b1e714998e6`. Mike asked us to proceed from the activity
plan and resolve the Git foundation decisively. This record describes source
integration and isolated validation. It is not a new deployment receipt.

## Baselines and ownership

Both preceding tasks completed. Their
[next-agent handoff](2026-09-06-next-agent-readiness.md) correctly identified that
canonical `main` was not the live bridge source. We retained that source and
integrated in `/Users/v/other/worktrees/astrid-activity-foundation`, branch
`codex/activity-foundation`, rather than modifying the selected release tree.

| Input | Identity and disposition |
| --- | --- |
| Canonical local main | `23df28497cf124a7dc1c5d7dfc7a92055e491eeb`; retain its two local research/self-study commits. |
| Independently verified remote main | `888c1708dcb3d4669e9219c2d0b9be1185984f4d`; Avado/ICP and artifact-prompt PRs were already merged. |
| Main integration checkpoint | `4c768362b3`; combines those histories without conflicts or losing either local commit. |
| Selected live source | `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf`, with the retained repair-worktree delta. Its 520 recorded build-input hashes matched at inspection. |
| Content-merge ancestor | `22709f3cccb84716b37fc274f525ee30bd4f10e2`; use source-level three-way results, not the live branch as a merge parent. |
| Maintenance ownership | Controller pause generation 372, actor `codex-activity-foundation`, no active steward run. The earlier usage-saving scheduler pause remains separate. |

The root owns Git/index operations; internal reviewers had disjoint source or
documentation paths. Other user-owned tasks were complete and no overlapping
editing process was found. Remote tips, both main working trees, the candidate
and the model repository were inspected. No foreign work was swept into a commit.

Before integration, the exact 86 non-runtime candidate delta files were copied
to a private archive with before/after SHA-256 checks. The archive, merge inputs,
and test logs are under
`/Users/v/.codex/artifacts/astrid-activity-foundation-20260906`.
The original selected source and `.runtime` stage remain in place.

## Content decisions

- Preserve live agenda/focus, actual ATTEND prompt allocation, signed self-control
  and envelope registry, Division/promotion, owner inquiry/concern/policy,
  source-preserving context, lifecycle/drain, stage identity, and learning-clock
  and target-reference machinery.
- Preserve main's full-history session resolver, typed session outcomes,
  bounded self-study contracts, precise probe interpretation, newer CPU-edge
  behavior, and root package/dependency upgrades. The live source lacks several
  of those newer main files; a directory replacement would have lost them.
- Twelve edge-script add/add differences were exact older blobs in main's
  ancestry. Retain the newer main versions, including their privacy, recovery,
  packaging, and artifact-name corrections.
- Retain main's root/storage dependency graph at 0.5.6. Do not restore removed
  optional SurrealDB code or older root tooling. Resolve the bridge lock from
  its merged manifest; root and edge lock changes reflect the protocol's added
  signing dependencies, without a wholesale dependency update.
- The shared protocol now includes the live self-control, volition and semantic
  body contracts. Edge ingress does not implement semantic body, so its hello
  explicitly withholds that capability while preserving legacy semantic delivery
  and receipts. This compatibility repair has a regression test.
- Include the version-controlled envelope seed and its matching clamp-grid test.
  The first source-only selection omitted `config/`; build-input reconciliation
  and the full suite exposed the omission. No mutable live registry was copied.
- Preserve the updated domain-boundary documentation, manifest and unchanged
  review ceilings. Add `.runtime/` to root ignores to keep local releases,
  checkpoints and activation transactions out of ordinary source inventories.
- Import seven curated prior-task handoffs and only the candidate's new feedback
  ledger entries, each visibly attributed as historical evidence. Do not import
  historical evidence directories, raw journals, signing keys or checkpoints.

The [source import receipt](evidence/2026-09-06-activity-foundation/source-import.json)
records reviewed paths, source-side hashes, resolutions, and integrated hashes.
It witnesses file provenance, not behavioral equivalence or deployment authority.

## Verification

These counts overlap where a focused test is also in a full suite; do not add
them to produce an inflated unique-test total.

| Check | Result |
| --- | --- |
| Main/remote merge | 7 source-preparation Python tests; 4 Rust artifact-prompt tests passed. |
| Complete bridge library test inventory | All 2,016 distinct tests covered: 2,014 passed in the isolated run, the source-alias fixture test passed after completing its source fixture, and the default-path test passed separately without path overrides. No test ignored or left failing. |
| Prompt merge | 7 retrieval tests and 172 provider tests passed, including the added ATTEND/Unicode/zero-budget/old-overflow regression. |
| Bridge build/lint | Library and binary checks passed; library Clippy with warnings denied passed. |
| Shared protocol | 53 tests passed; all-target Clippy with warnings denied passed. |
| Edge compatibility | Offline check and 3 WebSocket tests passed. |
| Retained edge scripts | 125 Python tests and the CPU-edge packaging fixture suite passed. |
| Deployment and source helpers | 105 tests passed across stage, activation, drain, selected-release launcher, graceful reload, Minime binding, boundary audit, wrapper and synthetic probe integration. |
| Probe/witness/preflight merge | 42 Python tests and 11 preflight self-tests passed. |
| Additional imported Python modules | 74 tests passed, covering projection/introspection, replay campaigns, proposal wrappers, Division and canary isolation. The production proposal-applier CLI was not executed. |
| Domain audit | Valid, zero violations; no exception ceiling increased. |

Tests used private stores, copied source-only introspection fixtures, fake control
calls and synthetic handles on ephemeral loopback ports. The default-path test
specifically asserts the absence of overrides, so it was executed separately.
Initial fixture failures and the omitted committed seed were resolved without
relaxing assertions. There were no live model calls, service signals, state
resets, forced transitions or deployment operations.

The additional module pass recovered five reviewed static assets: the felt
constellation policy, public codec corpus and embeddings, the frozen replay
campaign manifest, and the steward loop prompt. They are source inputs, not
activation instructions. The frozen manifest contains typed claim summaries and
hash/path references; its 35 owner-only source snapshots remain excluded, so
historical replay is not available from this checkout alone. Two historical docs
referenced by the loop prompt also remain outside this import. Synthetic replay
tests exercise the actual freeze/replay implementation with temporary inputs;
canary tests now keep positional checkouts inside their own temporary directory.

## Main and release remain distinct

The selected pre-existing stage is
`20260906-learning-clock-01`, with binary SHA-256
`6053aeb4f930cab21cd44de0977715c592bc2e229c81b2c0abbf0ee21da149e0`
and manifest SHA-256
`e6d0cc01b13993ffc931576b64fbfc99d39206184ba72d0147d40ca2169ee742`.
This integration preserves its source capabilities while adding main-side work;
it does not claim that the resulting source has already replaced the binary.
Any later release needs its own build-input manifest and sanctioned activation.
RASCII and prime_esn_wasm remain external dependencies; an Astrid commit alone
does not pin their source. Their recorded stage hashes remain the old release's
evidence, not an attestation for a future build.

Minime remains at feature HEAD `a9f85f3c74c3d8e1c996c3689fe5aef696dacf27` with
the preceding task's uncommitted Python/session/inbox work. The model repository
remains at `afc2931a657d1bd79a7076ece6310ee3d8f6ceba` with its existing local
changes. Neither sibling's dirty work was merged, restarted or represented as
loaded code by this pass. Minime adapter work requires its own reviewed source
checkpoint; the Astrid-first contract can proceed on this reconciled baseline.

## Canonical checkpoint

Canonical `main` now contains this reconciliation at `7b9f4d544d9661491eb9a57286daeca929e7d760`.
Its tracked tree matches foundation checkpoint `5bcb457fe6` except for preserving
the existing executable bit on `scripts/bridge_release_launch.py`; launcher bytes
are unchanged. The source import receipt describes that foundation, not later
feature edits. The three original discussion documents were preserved in an
explicit path-scoped stash before their reconciled versions were imported.
Remote main was independently rechecked at `888c1708dcb3d4669e9219c2d0b9be1185984f4d`.
No push occurred.

## Next feature step

**Subsequent completion:** the [runtime integration](2026-09-06-reading-mailbox-runtime.md)
and [verified rollout](2026-09-06-reading-mailbox-rollout.md) now implement the
episode described below. The foundation checkpoint and its original verification
remain unchanged; later source, live and review identities are recorded separately.

The [activity plan](../architecture/activity-continuity-and-inbox.md) remains the
throughline. Its first repair is typed, durable offered/committed reading
progress in the existing continuity log, followed by exact prompt-delivery
evidence, boundary-based inbox admission, and explicit return. Existing agenda
and ATTEND mechanisms are reused. No claim is made that this integration alone
implements that episode or closes the remaining
[preimplementation findings](2026-09-06-activity-continuity-preflight-findings.md).
