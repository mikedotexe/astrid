# Next-Agent Readiness: Main Is Not the Live Baseline Yet

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-next-agent-readiness.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Audit: 2026-09-07 around 00:18 UTC (September 6, America/Los_Angeles).
Actor: `codex-astra-interactive`. This is an inspection and handoff, not a
stabilization claim, deployment, new approval, or instruction from a being.

## Decision

The previous graceful bridge activation is verified and the system has continued
producing exchanges. The stronger condition requested by Mike is **not met**:
the repositories are not clean, the live candidate is not merged into Astrid
`main`, and the Minime/model repositories are on feature branches with local
changes. Begin read-only design work against an explicitly named baseline if
useful, but make integration/stabilization the prerequisite for a large shared-
tree implementation or another deployment.

Do not use a successful restart as evidence that current source, Git HEAD,
remote main and loaded processes are all equivalent. They are different objects.

## Verified Git State

| Repository or checkout | State at audit |
| --- | --- |
| `/Users/v/other/astrid` | `main` at `23df28497cf124a7dc1c5d7dfc7a92055e491eeb`; two local-only commits and five remote-only commits after fetch; one tracked modification and five untracked entries |
| Astrid remote `main` | `888c1708dcb3d4669e9219c2d0b9be1185984f4d`; independently read with `ls-remote`, then fetched without touching checkout/index |
| `/Users/v/other/worktrees/astrid-hebbian-clock-boundary` | `codex/hebbian-clock-boundary`, HEAD `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf`, plus inherited deployment and repair changes; 46 tracked modifications and 38 untracked entries before this note |
| `/Users/v/other/minime` | `codex/sovereign-daughter-runtime` at `a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`; feature tip matches remote; 11 tracked modifications and nine untracked entries |
| Minime `main` | `bcf5c6d2561f4e46e8454ee4542ff3fda61f8836`; current feature branch contains 31 additional commits; this is ancestry, not a completed merge review |
| `/Users/v/other/neural-triple-reservoir` | `feat/service-stack-and-multi-headed` at `afc2931a657d1bd79a7076ece6310ee3d8f6ceba`; three tracked modifications and six untracked entries; remote was not refreshed for this repository |

Untracked entry counts use Git's normal collapsed-directory status, not recursive
file counts. The canonical Astrid, repair-tree and Minime indexes were empty.
The model index was not separately inspected. No source was changed by this audit.

PR **18**, Avado/ICP preparation, and PR **21**, edge artifact prompt integrity,
are already in remote Astrid main. Do not attempt to merge them again. They do
not include the newly deployed learning-clock candidate. Local main's two
additional commits are `2675fa212b` (research framing) and `23df28497c` (bounded
self-study foundation); preserve and review them during reconciliation.

Remote-main versus retained bridge-base ancestry reports 65 versus 190 exclusive
commits. This is not a count of distinct missing features: history and content
both need review. The direct comparison across `capsules/spectral-bridge` and
`scripts` reports 303 changed files, 80,978 insertions and 45,654 deletions,
**before** applying the repair tree's uncommitted changes. Differences include
agenda, self-control, orchestration and deployment wrappers. This is not an
appropriate blind merge, wholesale checkout, or overwrite with one side.

## Live Identity

Canonical selector:
`/Users/v/other/astrid/.runtime/bridge-deployment/active.json`.

Selected stage:
`/Users/v/other/worktrees/astrid-hebbian-clock-boundary/.runtime/bridge-stages/20260906-learning-clock-01`.

- Bridge PID **56043**, replacing **98502** through acknowledged drain.
- Binary SHA-256:
  `6053aeb4f930cab21cd44de0977715c592bc2e229c81b2c0abbf0ee21da149e0`.
- Manifest SHA-256:
  `e6d0cc01b13993ffc931576b64fbfc99d39206184ba72d0147d40ca2169ee742`.
- Activation transaction **7c36fe0a2f7e4371bd71fc9d19d9614a**,
  `activated_verified`; exact stopped checkpoint and signed self-control lineage
  verified before admission, then a new saved exchange verified.
- Every recorded build-input file still matched its SHA-256 during this audit.
- Saved exchange count advanced to **191351**; model readiness was true, queue
  depth zero, no reported last-generation error, reservoir connected.

The full account, tests and after-stack receipt are in
[the rollout record](2026-09-06-learning-clock-live-rollout.md).
Do not delete, move or modify the selected source tree or stage merely to make
Git status cleaner. Runtime transactions contain private checkpoints and must
not be swept into a commit. Porting into an integration checkout must preserve
this running baseline until a tested, sanctioned replacement is verified.

## Remaining Runtime Work

1. **Fresh learning outcome observation remains open.** Four restored outcomes
   were durably retired; by saved count 191351 the queue was empty, the obsolete
   watermark null, and no fresh evaluated-pair log had been observed. Tests pass,
   but do not claim fresh live learning from retirement alone. Trace whether a
   semantic exchange is eligible and actually arms a baseline before investigating
   its next outcome. Preserve missing/expired/reference-change reasons; do not
   force a being Action or relax guards to obtain a success record.
2. **Artifact-scan latency is source- and profile-grounded.** The previous packet
   records the one-second sample in `btsp::signal::recent_owner_artifacts`, whose
   full directory scan precedes its six-artifact output limit. Investigate
   bounded indexing/incremental selection with recency, deduplication and owner
   semantics preserved. Do not delete evidence as a performance fix.
3. **Telemetry timing is not fully healthy.** Transport reconnects and
   `timing_ambiguous` remain observed despite passing stack presence/freshness
   checks. A coherent typed target/source/producer-epoch packet would improve
   provenance, but requires a coordinated producer/decoder rollout, not an
   assumed 68% default or undocumented protocol extension.
4. **A new fallback warrants bounded review.** After several live dialogues,
   exchange 191350 completed as `dialogue_fallback` at
   `2026-09-07T00:17:00.971302Z`. The log reports an Ollama fallback request
   transport failure at `00:16:54.449335Z`. Subsequent readiness was true. One
   fallback is not proof of a persistent outage or of a new regression; retain
   its route context and look for recurrence without inducing generation.

These observations do not invalidate the completed activation, nor do they
justify describing the whole stack as free of lingering issues. Separate
deployment integrity, voice/generation health, learning eligibility and felt
reports in the next verification.

## Ownership and Safe Integration Sequence

The earlier task `Prepare repo for Avado work` reported releasing Git ownership;
this audit found it not loaded. The recent architecture tasks were idle. A
separate Claude process was present in `/Users/v/other/reservoir-llm-research`.
These are momentary observations, not a lock or proof of no concurrent edits.
Claim a fresh stabilization window and recheck all owners before any staging.

1. Preserve the usage-saving scheduler pause. Controller maintenance was already
   released at generation 371; this audit claimed no lease or new pause.
2. Claim the controller stabilization window per `AGENTS.md`, inspect all three
   repositories, remote tips and cooperative sessions again, and read statuses
   twice. Keep foreign notes, runtime state and mixed-author changes distinct.
3. Review and archive the tested bridge candidate with explicit path ownership,
   source/test/evidence linkage and required report attribution. The uncommitted
   candidate includes inherited work, not just this session's clock module.
4. In a **separate integration checkout**, reconcile the candidate, the two local
   main commits and current remote main. Preserve live agenda/ATTEND, self-control,
   Division, prompt, study and deployment capabilities alongside edge/device work.
   Review sensitive-file exclusions before importing historical evidence.
5. Resolve Minime and model source ownership and appropriate main branches
   separately. Do not merge or deploy their uncommitted work merely because it
   exists, and do not infer loaded Python source from the files currently on disk.
6. Test the integrated state, including the full bridge suite, lifecycle/manifest/
   handoff helpers and touched domain tests. Review every staged diff and keep the
   index under one owner. No broad staging, destructive cleanup or history rewrite.
7. Establish an explicit main-to-release mapping. A Git merge alone does not change
   a running binary. If the reconciled runtime inputs differ, build a fresh stage
   and use the sanctioned graceful wrapper, exact checkpoint and signed-handoff
   gates, then verify actual source hashes, services and natural saved exchanges.
8. Declare the larger-feature baseline ready only with exact commit/release IDs,
   clear remaining exceptions, no unowned index changes and a durable handoff.
   Remote publication is a separate operation; this audit did not push anything.

Existing foreign architecture notes under canonical Astrid, including
`docs/steward-notes/2026-09-06-activity-continuity-preflight-findings.md`, are
relevant inputs for the proposed larger feature, not changes owned by this audit.
Their source-confirmed findings must be rechecked against the reconciled live
lineage before being treated as reproduced or repaired behavior.

## Audit Actions

Read-only process, readiness, Git and log checks; one ordinary
`git fetch --no-tags origin main` to refresh remote references; this handoff note.
No staging, commit, merge, push, source mutation, service signal, state reset or
automation update was performed. No full test suite was rerun for this status
audit; the preceding rollout's test receipt remains the applicable evidence.
