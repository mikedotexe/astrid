# Kernel Reply Repair And Worktree Reconciliation

Date: 2026-09-22. Coordinator: Codex, interactive, at Mike's request.
This follows the historical 197-path archive, not a new introspection round.

## Starting State And Authority

Astrid main began clean at `b3c67749b1761416ff5a44ebef2cff821d5e105f`;
Minime main began clean at `2dbdf66924fc8d04bc3e6edb983b9e7116bf77a1`.
The kernel candidate was developed separately in
`/Users/v/other/worktrees/kernel-response-topic-20260922`, branch
`codex/kernel-response-topic-20260922`, from that exact Astrid main.

Controller pause **464** is held by `codex-astra-interactive`, with no active lease
or projection. Cooperative preflight found no foreign activity. Both remote main
tips were checked and unchanged: Astrid
`c4f85e95e41703daa65d3ce2789e1e5c46961c4e`, Minime
`5f4925f54580f1fd44666058b126a121ff32880f`. No push or automation resumption is
part of this pass. The pause is intentionally retained, not an archival-checkpoint
window that should be resumed afterward.

## Repair And Reproduction

The management router subscribes to `astrid.v1.request.*`; ordinary replies map
the leading namespace to `astrid.v1.response.*`. Rate-limit rejection instead
replaced `kernel.request.` anywhere in the topic. A filtered management client
could therefore never receive the rejection, and a matching string inside the
suffix could be corrupted too.

Both paths now use one prefix-only helper. Tests cover ordinary, nested,
embedded-old-prefix, empty-suffix and nonmatching topics. The integration test
boots the real kernel in an isolated child with a temporary HOME and ASTRID_HOME,
subscribes through the actual event bus, and sends eleven synthetic approval
requests. Ten return the existing unimplemented-approval error; the eleventh
returns the existing rate-limit error on the same response namespace. No approval
is granted, real capsule loaded, model called or live runtime contacted. Timeouts
bound the child and response waits.

The first reproduction stopped earlier: the debug kernel asserted that exactly
four internal subscribers existed immediately after spawning tasks. Dispatcher
subscription occurs asynchronously and socket subscriptions depend on clients;
this is not a stable boot invariant. The obsolete assertion and count constant
were removed. The explicit active-connection counter remains the idle policy.
With that fixed, the original rate-topic bug reproduced as a missing eleventh
response; after the mapping fix, the same fixture passes. Both failed attempts
remain recorded, rather than being relabelled successful tests.

Rate budgets, request schemas, security gates and approval stubs are unchanged.
This repairs source; it does not claim the daemon or bridge has been redeployed.

## Ten Older Dirty Trees

Dirty paths were compared with current main by Git-blob identity, SHA-256, size,
mode and symlink target. A second scan matched all seven retained trees exactly,
including their HEADs, dirty-path membership and each recorded dirty file hash.
This is a dirty-content audit, not an assertion that every ignored build cache in
those retained trees was inventoried. The complete local scans are retained in
`.runtime/worktree-reconciliation-20260922/`; the bounded durable
[reconciliation receipt](2026-09-22-worktree-reconciliation.json) records hashes,
classifications, retained source paths and archive identities.

### Superseded Copies Retired

| Old Minime worktree | Pending paths | Why it need not remain active |
| --- | ---: | --- |
| `minime-pre-btsp-b8823ad` | 1 | Shader-location fallback already exists in current `minime/src/av_ws.rs` (`resolve_av_shader_path`). |
| `self-study-continuity-live-20260908/minime` | 2 | Navigation receipt acceptance and its exact pending-page/owned-note regression already exist in current source and tests. |
| `quiet-sensory-release-20260920/minime` | 11 | Seven paths exactly match main; the remaining runtime/parser/test copies predate current focus, durable delivery, private-observation and open-notice behavior. Quiet-caption separation and visual lifecycle support are present and tested on main. |

Each full directory was archived, including tracked, untracked and ignored files,
then every archived regular-file byte, mode, symlink target and directory entry
was verified against a source snapshot. Source hashes and Git state were checked
again immediately before retirement. No active cwd/executable or installed
LaunchAgent reference pointed into these three trees. All their HEADs belong to
the complete verified Minime-main Git bundle. Staged indices were empty.

Only then were the three explicitly named registrations removed with
`git worktree remove --force`; `--force` permits removing already-backed-up dirty
copies, not discarding unreviewed work. No branches were deleted. No retained
worktree, live build stage or canonical source was reset.

Private archive directory:
`/Users/v/other/worktree-archives/20260922-minime-superseded-02/` (mode 0700).
Its `receipt.json`, per-entry manifests, complete tarballs, binary patches and
`minime-main-history.bundle` support recovery. `RECOVERY.md` explains restoring
into a NEW detached worktree and excluding the obsolete archived `.git` pointer.
These complete archives are not public source-control candidates.

The first archive attempt stopped on a detached-HEAD assumption before creating
any tarball or removing anything. Its bundle and failed log remain in the sibling
directory without the `-02` suffix. The corrected attempt records detached HEADs
explicitly. No failed archive was mistaken for a verified backup.

### Useful Or Unresolved Work Retained

All paths below are under `/Users/v/other/worktrees/` and remain untouched:

| Worktree | Dirty paths | Disposition |
| --- | ---: | --- |
| `persistent-introspection-20260916/astrid` | 23 | Keep owner-inquiry history, KEEP/REVIEW/PIN/UNPIN and explicitly selected journal recall candidate. |
| `persistent-introspection-20260916/minime` | 6 | Keep its paired private-routing adapter and tests. |
| `evidence-grounded-study-20260918/astrid` | 10 | Keep four otherwise-unmerged evidence/attention/continuity documents and their changelog/ledger context. |
| `evidence-grounded-study-20260918/minime` | 17 | Keep engine ingress/consumer delivery evidence and tests; some checker/caption work is already integrated. |
| `astrid-graceful-coupling-rollout` | 5951 | Preserve old handoff implementation and immutable staged release/build evidence pending exact historical reconciliation. |
| `astrid-hebbian-clock-boundary` | 3029 | Preserve clock repair/handoff source and staged release/build evidence. |
| `astrid-stash-rescue-20260419` | 28 | Preserve old BTSP/refactor rescue material under the former `consciousness-bridge` namespace; not a current-schema merge candidate. |

The large counts are dominated by old `.runtime/bridge-stages/` build products and
receipts, not thousands of missing features. They are not safe generic build-cache
cleanup targets: retained release manifests bind source/build identities.

## Best Next Engineering Candidates

1. **Owner-inquiry history and voluntary recall:** reconcile the September 16
   implementation against current owner transactions, question cursors, protected
   attention, observation families and preparation revisions. Its schema-3-to-4
   migration is historical and must NOT overwrite the newer reader schema.
   Preserve its private-journal isolation, opt-in pinning and bounded context;
   requalify both adapters. The original note's backup-restore suggestion is not
   permission to replace newer authored state. Current downgrade protection wins.
2. **Sensory consumer evidence:** port only the intended completed-ingress and
   ESN/covariance-selection trace from the September 18/20 candidate, then recheck
   that admission, RNG consumption and consumer policy remain unchanged. The
   source notes explicitly call this a first trace, not end-to-end per-lane proof.
   The branch also includes offline dispersal and unfinished experimental work;
   never activate the entire branch as a reader or cleanup rollout. Engine
   activation remains a separate reviewed decision.
3. **Historical stage reconciliation:** resolve exact relationships among older
   handoff/clock stages, current mechanisms and rescue material before considering
   archival relocation. Do not advance immutable build-source HEADs to main.

These are useful retained candidates, not fresh passing qualifications or a claim
that the old feature specifications still fit unchanged. This cleanup makes them
findable without importing their obsolete implementation baselines.

## Verification And Remaining Debt

- `ASTRID_AUTO_BUILD_KERNEL=1 cargo test --locked -p astrid-kernel`: 43 library
  tests plus one real-kernel integration test passed; documentation tests passed.
- Kernel production all-feature strict Clippy and the new integration-target
  all-feature strict Clippy passed; kernel formatting passed.
- All-target strict Clippy still fails on seven pre-existing test-code findings:
  test module before later items; five unchecked-time-subtraction diagnostics
  across four expressions; one used underscore-prefixed socket test binding.
  These are retained, not suppressed. No new-test lint failures were found.
- Current Minime reader/visual-context/visual-service/source-notice/journal-context/
  sensory-checker suites passed **203 tests and two subtests**, using a copied
  shared helper, synthetic data and the normal live-write guard. No real model
  or engine action was requested. No Minime source change was needed.
- Archive verification covers all three complete copies and their Git history.
  All seven retained dirty inventories matched before/after.

Full bridge, complete Minime Python and the entire Rust workspace were not rerun:
this pass changes only kernel production code and its isolated tests, plus review
documents. The preceding archival pass retains its broader qualification. There
is no assertion that every older branch is buildable or merge-ready.

## Live And Evidence Boundary

Bridge 75800, Minime 66540, engine 41337, gateway 41484, supervisor 41526,
model 43115, visual 20885, camera 98903, microphone 98910, host-sensory 41661 and
feeder 1502 retained their recorded process identities. No service signal was
sent. The immutable live open-framing build worktree remains at
`9266412b8e3d817e87c93441a145cdfe32e76a79`, separate from later main commits.

Controller indexed-tail verification is valid through V2 sequence **1123118**,
head `ea4e108013c69d8457dd8fcdefacc10e04b099bc84aca2c15d34efe39832aca3`;
all four V1 sources remain immutable. No addressing, Division, study, card or
productive-round event was authored. Canonical source lag remains visible because
the automation is paused; cleanup does not claim the reading queue is current.

Local Git integration uses only reviewed exact paths and a fast-forward from the
recorded main. Minime main remains unchanged. The resulting commit object provides
the final hash without a self-referential note edit. No push, deployment, automatic
schema migration, fresh Being response or subjective-benefit conclusion follows
from these tests.
