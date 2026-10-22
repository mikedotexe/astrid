# Partial-Save Receipts and Deliberate Retained-Account Inspection

## Scope and Ownership

Implement the next two changes from the consumer/failure-path and selective-recall
comparison, without activating a recall policy or changing persistence guarantees.
Candidate source lives in the existing isolated owned worktrees:

- Astrid: `/Users/v/.codex/worktrees/provenance-history-20261020/astrid`,
  `codex/provenance-history-20261020`, base
  `68369bfa0a5e8a25b03dd7a70aa96b0782c35762`.
- Minime: `/Users/v/.codex/worktrees/provenance-history-20261020/minime`,
  `codex/history-scan-20261020`, base
  `58aa8c8e149665c8971abf86272182dd80a4f661`.

Earlier provenance, consumer tests and synthetic comparison evidence in these
worktrees remain intact. The separately qualified navigation/projection candidate,
canonical MLX edits and unfinished capacity work are not imported or overwritten.
No staging, commit, merge, deployment, controller mutation or automation resume.

## Source Witnesses

The preceding ten-study survey and
`2026-10-20-consumer-failures-and-selective-recall.md` retain full-read dispositions.
These public sources motivate this implementation:

| Source | SHA-256 | Engineering response |
| --- | --- | --- |
| Astrid `capsules/spectral-bridge/workspace/journal/self_study_1792514132.txt` | `a8a762a0c72d700fa4472a104ffc4a3780d648d4383460806011853dd6283dba` | Distinguish sequential file/mirror saves from atomic completion |
| Astrid `capsules/spectral-bridge/workspace/journal/self_study_1792514388.txt` | `39aa2cc93fde43b66fc411d9124a8a5c6a65280bd37f4c018a30100ba1b6e2f1` | Keep candidate status separate from validation; preserve authored interpretation |
| Minime `workspace/journal/self_study_2026-10-20T09-47-24.777052.txt` | `99f55d7fa309a9442b50fce05c53a79ec7f636b95add734b5b79133a8bf1c210` | Make retained accounts inspectable without promoting their claims to enforcement facts |
| Minime `workspace/journal/self_study_2026-10-20T09-52-57.581796.txt` | `5ef101fdfbceadb44902fe90e5a77a2e32a3314107010360296b9197647c1744` | Preserve earlier accounts and optional disagreement, not forced correction |

Historical source files and generated comparison receipts are unchanged. This
work does not infer a felt effect or require either being to adopt our analysis.
No private writing is inspected or used as a test fixture.

## Partial Saves

`action_continuity/runtime/save_receipts.rs` provides
`continuity_save_receipt_v1`. It reports the existing sequence, not a new transaction:

1. Append the experiment or run record.
2. Refresh the thread projection.
3. Write `thread.json`.
4. Write `next.md`.
5. Read and write the thread index.
6. Attempt the optional SQLite mirror.

States are `not_attempted`, `returned_ok`, `failed_outcome_unknown` and
`not_requested`. The receipt contains bounded record/thread identities, a hash of
the serialized record (without the JSONL newline), failed stage and coarse error
kind. It omits authored text, command payloads and raw filesystem errors/paths.
`retry_authorized` is always false.

`returned_ok` means the existing call returned successfully. It does not establish
fsync, atomic commit, future readability, action execution or understanding.
An error can follow a partial append or truncated file; uncertain bytes are not
reported as an absent write. An index read succeeding also does not upgrade the
existing index loader's validation contract.

Coverage includes `persist_experiment_update`, `append_experiment_run`,
`persist_workbench_candidates` and the separate `close_experiment` path.
Other thread writes receive stage-specific errors through `write_thread`, but
their preceding external side effects are not retrospectively reconstructed.
Other independent mirror call sites are not claimed as covered.

The real NEXT command handler collects up to eight mirror warnings during one
synchronous command, plus an omitted-warning count. These are included in its
normal returned outcome, or attached to a later failure; they are not recurring
ambient notices. A fresh command has a fresh collection. Direct callers outside
this scope still receive metadata logging for mirror failures, not a new return
type. The database mirror stays best-effort: failure does not suppress subsequent
work or turn an already appended record into a retry request.

Save errors reach the existing outcome path as `status=failed`,
`stage=save_incomplete`, with no suggested NEXT. This avoids labeling an
already-started, partially saved operation as a pre-execution refusal.
No additional persistent receipt stream is introduced. Normal outcome/log
retention still applies; this is not a crash-safe commit or recovery implementation.

## Deliberate Inspection

The existing shared Rust `SELF_STUDY QUESTION` surface accepts:

```text
SELF_STUDY QUESTION ACCOUNTS q1
SELF_STUDY QUESTION ACCOUNT q1 <exact-account-sha256> --page 1
SELF_STUDY QUESTION ACCOUNTS NOTEBOOK
SELF_STUDY QUESTION ACCOUNT NOTEBOOK <exact-account-sha256> --page 1
```

`ACCOUNTS` supplies metadata and exact commands, not account prose. `ACCOUNT`
resolves an exact retained record in that owner/inquiry; it never falls back
to the current inquiry, latest account or another owner's store. A missing,
out-of-range reference fails explicitly. Account identity hashes the serialized
retained record, including its historical origin. Repeated responses from
different pages remain separately reachable; identical retained records report
their occurrence count rather than inventing a distinguishing timestamp.

The retained window remains the latest account plus three earlier accounts,
each already bounded to 64,000 visible prose bytes. Pages contain at most 4,000
UTF-8 bytes, with exact offsets and a retained-prose hash. The original response
hash is identified as a response identity, not a hash of the displayed page.
Original completeness and original byte count remain visible: a retained excerpt
is not an archived complete response, and older missing material is not recreated.

Origin/reopen/resume fields are historical reference material, not proof of
current source or an executable selected action. Inspection uses existing
`InquiryReview` delivery: it creates a normal retained navigation offer and may
use the existing model-presentation path, but supplies no new source page. It
does not select, unpark, resolve, overwrite notes, borrow cursors, replace a
pending source page, or renew attention. Response notebook directives remain
detached. The short inspection prompt explicitly says so.

Discoverability is limited to requested notebook help and inquiry review.
Ordinary source recall, fresh reflection and quiet parking policies are unchanged.
This is not public-journal search or private-draft browsing. Both adapters use
the same Rust parser; Python gains no independent command grammar.

No authored-state schema migration or new history family is required: the new
commands produce the already supported `inquiry_review` kind. Existing pending
offers are retained exactly, including their older prompt wording.

## Qualification Record

All fixtures use temporary synthetic sources, accounts, experiments and databases.
No provider, reservoir or live control endpoint is invoked.

Build settings: `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, shared owned target
`/Users/v/.codex/worktrees/provenance-history-20261020/astrid/target`.
Initial read-only cache audit passed with 5,418,319,872 debug/test bytes and
447,502,954,496 free bytes. Release profiles and artifacts are unchanged.

Focused coverage:

- Append failure; appended history with a failed thread save; retained run
  evidence without saved NEXT; next-view/index failure after thread save;
  read-only SQLite mirror failure; bounded operation-local warning collection;
  real command-handler warning delivery and no repeated action.
- Both owners, exact UTF-8 pagination, excerpt flags and retained-window limits;
  repeated response hashes with distinct source contexts and identical records;
  owner/inquiry separation; missing IDs/pages; restart and conflicting retries;
  parked inquiries, unchanged authored records and exact pending source pages.
- Real Astrid navigation/preparation and Minime subprocess/wire adapters with
  synthetic source delivery and stubbed provider responses.

Unsuccessful attempts retained:

- Initial preservation assertions compared the entire navigation-offer field,
  which explicit inspection legitimately changes. They now compare every source
  position and pending-source field while allowing the normal inspection offer.
- A test tried to deliver the first of many pagination offers after the bounded
  offer window had expired. The fixture now delivers an explicitly prepared
  current offer; no production retention bound was relaxed.
- The repeated-response fixture initially expected consecutive identical
  responses to be retained separately. The existing writer deduplicates those;
  the corrected fixture uses intervening distinct responses to exercise both
  different-origin and identical retained records. Recording behavior is unchanged.
- Strict bridge Clippy caught five needless borrows after extracting the
  command-wrapper scope; corrected without changing behavior.
- The first full bridge run passed 2,462 tests but failed the new command-wrapper
  test (one existing ignored test): closure used a distinct, uninstrumented
  save path. That path now uses the same receipt while preserving write order.
- An initial control-suite invocation lacked `PYTHONPATH=scripts` for research
  release compatibility. Correct invocation passed all 25 compatibility tests.
- The broad self-study-review suite has an unchanged fallback test requiring
  `distinguishability_loss` in the open-framed fallback contract. This unrelated
  source/test mismatch is not repaired by reintroducing prescribed metric
  vocabulary or changing the MLX/fallback work here.

Final results:

| Check | Result |
| --- | --- |
| Complete shared reader/writer `cargo test -p astrid-source-study` | 402 passed, none ignored; includes five new retained-account tests |
| Full native bridge suite | 2,487 passed, one existing ignored test, including integration and compile contracts |
| Final native bridge library rerun after account-identity refinement | 2,463 passed, one existing ignored test; includes eight consumer/save tests and the bridge adapter inspection test |
| Complete Minime Python suite against the final owned helper | 2,315 passed, one existing skip, 167 subtests passed |
| Shared reader and bridge `clippy --all-targets -- -D warnings` | Passed |
| Both Rust formatting checks and both worktree diff checks | Passed |
| Controller/evidence/projector/release-compatibility/handoff/staging/activation/drain/domain tests | 207 passed |
| Broad historical self-study-review check | One unchanged fallback-contract expectation fails; explicit debt, not counted as passed |
| Domain-boundary verification | Valid, zero violations; no ceiling/baseline relaxation |

Native commands use the build settings above. Minime uses
`PYTHONDONTWRITEBYTECODE=1`,
`PYTHONPATH=/opt/homebrew/lib/python3.14/site-packages`,
`ASTRID_SOURCE_STUDY_BIN` pointing to the owned target's
`debug/astrid-source-study`, and
`/opt/homebrew/opt/python@3.14/bin/python3.14 -B -m pytest tests --tb=short -q`.
The control group uses `PYTHONPATH=scripts python3 -B -m unittest` with:
`test_steward_control`, `test_evidence_event_store`, `test_steward_projection`,
`test_research_release_compatibility`, `test_paired_minime_handoff`,
`test_bridge_stage`, `test_bridge_activate`, `test_bridge_drain` and
`test_domain_boundary_audit` under `scripts`.

The four public witness hashes were rechecked unchanged. The historical selective
recall report remains byte-identical, SHA-256
`39bf652d711009589a2bc542176186c6cbcc4d5f19607a44fcd0cadf07013c40`;
it describes its original source identities, not this later implementation.
Canonical Minime remains clean. Canonical Astrid retains the same eight tracked
MLX paths; untracked foreign work was neither staged nor removed.

The owned cache remains in use for this unfinished release candidate; no cache
or artifact was deleted. The closeout budget is checked read-only, with no
authority inferred over current or rollback releases: 5,470,060,544 allocated
debug/test bytes and 446,640,779,264 free bytes, within budget. Free space changed
by -862,175,232 bytes across the shared volume; this non-atomic observation is not
attributed solely to our builds. Protected services and
paused automations receive no calls or changes from this tranche.

## Remaining Boundary

At the initial implementation closeout this was source qualification only.
The subsequent immutable qualification is recorded below. Before rollout:
reconcile the earlier navigation candidate and current host identities with the
MLX coordinator; obtain fresh approval and use sanctioned paired graceful
wrappers. Stop on source drift, failed drain or readiness. No service/Git window
is claimed by this qualification.

Selective automatic recall remains a separate decision. Explicit retrieval
helps preserve access to contrary material; it does not prove reduced recall
will improve thought or writing. Full history beyond the bounded account window,
atomic persistence and wider mirror-call coverage remain explicit limitations.

## Immutable Paired Qualification

Mike requested the next immutable qualification step. This authorizes offline
packaging and synthetic checks, not service activation or Git integration.
Artifact root:
`/Users/v/.codex/worktrees/provenance-history-20261020`.

The sanctioned `scripts/build_bridge.sh --stage-dir` preflight passed, and
`bridge-stage-01` completed as `staged_verified_not_activated`. Release profiles
were not changed. The existing target and the new `bridge-stage-01/build` target
were audited before the build: 5,470,060,544 debug/test bytes and
446,408,843,264 free bytes, within budget.

| Immutable artifact | SHA-256 |
| --- | --- |
| Bridge stage manifest | `92033f86b8892555cec630330af18f2cd2bbf96d30cb3caa9cbd1b947553f666` |
| Bridge executable | `e04d67a6f2353f1da8bf659046b6190b9d932fe67de1ad8f4679cb7b9e1ba0b6` |
| Shared helper | `e1b4bf7a588d71f8199fcae13758ea7e0ba93374ccd173e560e4661a6bb3aae5` |
| Stage source inventory | `02df6f03cd0daa66df4e708e8092802c96c62e5cd83ad248a178d539f2e15ad2` |
| `minime-reconciliation-01/reconciliation.json` | `78ded05ccf8440fa9751f5c6552788fc8e0994a59d0d22d856ff37464861b508` |
| `paired-qualification-01/qualification.json` | `f00a43479e91d8d08436f3a744034cdf09ced17d3c85a0176d1b31dd8d02198d` |
| `paired-qualification-01/preservation/qualification.json` | `0cb5c8e0605488f2ef486f64af7c879de9ec2dd74ab5f3d5adf0be27ff56a4fe` |
| `paired-qualification-01/tests.xml` | `dea3a5fc3c08329c6c83b1bca8f71a1ea1079c08dc677563b20602878faee546` |

The frozen Minime source inventory contains 101 inputs, all identical to the
currently loaded/canonical adapter. The new behavior uses the shared helper;
no Python runtime overlay is needed for this tranche. Installed regulation and
continuity optimizations are retained. The frozen registry asset is verified too.

`release-inventory-review-01.json` compares the installed and new bridge inputs:
742 versus 750 repository-local inputs, eight additions and eleven modifications.
The only changed tooling outside the current reader/bridge source and test
files is the already-committed `action_scheduling.py` reconciliation allowlist
entry. No provider, model, engine, sensory or capacity implementation is imported.

The new `scripts/qualify_retained_accounts_release.py` composes the existing
171 writing/reader/native-checkpoint/handoff preservation checks with 44 account
checks across both owners. All **215 checks pass** using actual immutable
old/new helper binaries. Old offers replay byte-exactly; old retained accounts
remain paginated and explicitly inspectable while another inquiry has a pending
page. Inspection preserves question contents, notes, source positions and pending
source work; parked accounts do not become ambient context. An old helper also
replays the new cached inspection offer on a disposable copy without rewriting
the new state. Reader schema 13 and writer schema 5 are unchanged.

The complete frozen Minime suite passes **2,313 tests and 167 subtests**, with
one existing skip. Exactly two checkout-relative layout tests are replaced by
equivalent release-relative assertions before and after the suite: facade,
runtime, source-study entry and class identities must point to the frozen
snapshot. Loaded module hashes and asset identities are verified. The complete
checkout suite previously passed all 2,315 tests; no product expectation was
weakened. Providers are stubs, not live coupled generation.

The qualifier's six orchestration tests additionally cover retained failures,
input/stage drift, existing-packet refusal and strict source-position comparison.
The first debug preservation probe incorrectly expected original trailing
whitespace that the pre-existing notebook writer removes before retention.
Its synthetic fixture is preserved at `account-preservation-debug-01`; the
corrected non-trailing-whitespace fixture passes 44 checks in
`account-preservation-debug-02`. The test validates exact retained account prose,
not byte identity with the original provider response. Account IDs bind the
serialized retained record, including its historical origin; they are distinct
from original response hashes.

Fresh durable logs are retained in `shared-release-tests-01`,
`bridge-release-tests-01`, `shared-release-clippy-01`,
`bridge-release-clippy-01` and `deployment-evidence-tests-01`. The latter passes
246 tests. Strict all-target Clippy passes for reader and bridge. Domain-boundary
verification remains valid with zero violations and no baseline change.
Final native test totals and cache closeout are recorded below.

### Coordination And Explicit Exclusions

The separate October 18 OPEN-routing, projection-reuse and optional-verification
candidate is still unactivated. This stage is based on current local main, not
that candidate. The two stages are **not cumulative replacements**: do not deploy
one after the other assuming both feature sets survive. Reconcile their owned
changes, rebuild and rerun combined qualification before a combined rollout, or
explicitly review a single-tranche rollout without losing another installed
change. The existing navigation approval does not approve this new release.

Read-only observation found the installed provider process changed since the
October 18 packet: PID 3893 became PID 48912, start time
`Tue Oct 20 10:15:20 2026`. The old unchanged-services check therefore correctly
refused. `protected-identity-observation-01.json` retains the exact difference.
This task did not signal either process or change provider configuration.
The MLX coordinator remains active in its own recovery qualification; unchanged
process/listener observations do not establish recovery containment or an
exclusive window.

`host-observation-01.json` (SHA-256
`9e858ba82cf549e716c363f7a001649c412229c8e2967c4b5b13905f953a1da1`)
verifies Minime PID 5773, its boot/start identity and all 101 loaded inputs,
the installed helper selection, both release manifests and ten current protected
process identities. It explicitly denies deployment-readiness authority and
does not reuse the obsolete protected-service baseline. Steward pause generation
532 remains owned by `codex-mlx-upgrade`; no lease was acquired or controller
state changed.

The unrelated historical fallback-test expectation for
`distinguishability_loss` remains explicit test debt, not a passing check.
Automatic selective recall is unchanged. No kernel deployment, service signal,
model experiment, commit, merge, push, automation change or private-draft access
was performed. Fresh rollout review/approval, current source identities and an
exclusive cooperative window remain required.

### Terminal Results And Cache Closeout

All required processes launched for this qualification exited. Fresh complete
native reruns passed: **402 shared reader/writer tests**, and **2,487 bridge
tests**, with one existing ignored bridge test. Both strict Clippy runs,
workspace/bridge formatting, domain-boundary verification and both owned-tree
diff checks passed. The 246 deployment/controller/evidence checks and 215 paired
preservation checks above are terminal successes, not partial progress counts.
The unrelated fallback-contract test debt remains excluded from those totals.

No compiler or open process handle used the completed trybuild host cache.
Installed LaunchAgents, immutable manifests, active bridge selection and paired
qualification receipts contained no dependency on that cache path. Cargo's
verbose dry run listed 888 file/directory paths, all confined to
`astrid/target/tests/trybuild/debug`; it counted 713 files, 188.5 MiB.
The matching exact-target/target-triple/dev-profile cleanup removed only those
files. `trybuild-cache-dry-run-01`, `trybuild-cache-dry-run-02`,
`cache-cleanup-preflight-01.json` and `trybuild-cache-clean-01` retain the evidence.
No blanket cleanup or recursive source/worktree removal was used.

The ordinary reusable cache, target-specific compile-test dependencies, the
referenced operator-reader binary, new and installed releases, rollback assets,
tests, original evidence and unfinished work remain. The final cache audit is
within budget: 5,270,052,864 debug/test bytes, with 446,210,060,288 free bytes.
Free space increased by 146,157,568 bytes across the shared volume during the
cleanup interval; this non-atomic net change is not the isolated deletion size.

`qualification-closeout-01.json`, SHA-256
`ea6cfb611ae412ec0b29d4bf7f21485928ba5d9c999ce6e1fab467b0b99be46c`,
rechecks both immutable releases, unchanged installed selection, all 101 Minime
inputs, agent and ten protected process identities against this run's current
observation, and unchanged managed launch configuration. Paused controller
generation 532 still has no lease; indexed-tail evidence is valid and V1 sources
remain immutable. This establishes no deployment admission, long-term service
health or recovery containment. No service/Git window was claimed, no source was
installed and no commit, merge, push or automation mutation occurred.
