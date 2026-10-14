# Bounded Build Cache Housekeeping

## Outcome

Mike authorized removal of unused build/test caches while preserving live
releases, rollback assets, authored history and unfinished work. This was an
interactive maintenance pass, not a productive introspection round or deployment.

On October 14, 2026, free space on the shared 2 TB APFS container rose from about
**3 GiB to 210 GiB**, an observed net increase of about **207 GiB**. A post-cleanup
audit sample reported **225,621,454,848 free bytes**. Background services continue to
write, so this is a measured interval, not a guaranteed attribution of every byte.
Cargo's logical removed-byte totals include hard-linked/shared build artifacts
and must not be added together as physical reclaimed storage.

The separately authorized older-worktree/restore-test pass below subsequently
raised available space from **209.29 GiB to 356.42 GiB**, a further observed net
increase of **147.13 GiB**. The first-pass figures above remain historical rather
than the current available-space reading.

No restart, deployment, source rollback, Git cleanup, model deletion, database
compaction, journal deletion, worktree removal or automation resumption occurred.

## Reviewed Cleanup

Both canonical repositories started clean at Astrid `822f50ce43980568726990e7a35dc3502e059c21`
and Minime `68ae81722079bac0c4470a117a8d6271ed5e6c08`. Both existing build preflights
reported clean source and no foreign activity at the start. Controller pause
generation **519** remained in effect, with no active lease. No cache path was
tracked by Git. Installed launch agents and open process files did not reference
the selected debug/test scopes. Native kernel and engine use release binaries;
the bridge and shared helper use an immutable stage outside these caches.

Every operation used Cargo's explicit dev profile, `--locked --offline`, an exact
target directory and a preceding dry run. No target root was removed wholesale.
The table records Cargo's own completion counts, not physical disk recovery:

| Exact target directory | Profile | Removed files | Cargo logical total |
| --- | --- | ---: | ---: |
| `/Users/v/other/astrid/target` | dev | 366442 | 84.8 GiB |
| `/Users/v/other/astrid/capsules/spectral-bridge/target` | dev | 477842 | 230.4 GiB |
| `/Users/v/other/minime/minime/target` | dev | 366920 | 54.0 GiB |
| `/Users/v/other/astrid/target/tests/trybuild` | dev | 1126 | 362.2 MiB |
| `/Users/v/other/astrid/target/tests/trybuild/aarch64-apple-darwin` | dev | 9683 | 3.7 GiB |
| `/Users/v/other/astrid/capsules/spectral-bridge/target/tests/trybuild` | dev | 2043 | 687.8 MiB |
| `/Users/v/other/astrid/capsules/spectral-bridge/target/tests/trybuild/aarch64-apple-darwin` | dev | 20168 | 15.0 GiB |

Command shape (used with the matching repository's Cargo manifest):

```sh
cargo clean --locked --offline --manifest-path /absolute/repository/Cargo.toml \
  --profile dev --target-dir /exact/reviewed/target --dry-run
# Only after reviewing use, ownership, preservation and the dry run:
cargo clean --locked --offline --manifest-path /absolute/repository/Cargo.toml \
  --profile dev --target-dir /exact/reviewed/target
```

The native trybuild target required its own explicit target-directory operation:
passing the native triple via `--target` did not select that nested cache in this
Cargo version. The verbose dry run exposed this; no broader deletion was used.
Generated trybuild project manifests/sources were preserved outside `debug`.

A different agent began a `reservoir-core` test run under
`/Users/v/.codex/worktrees/tauri-checkpoint/reservoir-llm-research/desktop/ReservoirScope`
during this pass. PID/working-directory inspection established a separate target
scope. No process was terminated and no worktree cache was cleaned. The older
`target/staged-archive-check/debug` cache remains at 3,603,468,288 allocated bytes
pending its own provenance/use review. Capacity/provider candidate caches and
all unfinished or foreign worktree source and evidence remain in place.

## Preservation Checks

The immutable current and previous stages both passed `bridge_stage.verify_stage`
before and after cleanup, including their packaged executable/helper hashes:

- Current: `/Users/v/other/worktrees/writing-development-20261013/bridge-stage-01`;
  manifest `daacd50c39ce33b77b7c2c67525a5f2a52ed8e89c987e2f868e515d525d8819c`.
- Previous: `/Users/v/other/worktrees/writing-context-refinement-20261013/bridge-stage-01`;
  manifest `31e2330ca8472203225b0e9ef62b2dfc761e95e395c0536d066f4a9d7214ca19`.
- Kernel release SHA-256 remained
  `b59ce913f978b7e1b579296a9aa6cd4e20468d2fc5355e7bd32669d047f6135b`.
- Engine release SHA-256 remained
  `0610800dd57bf34287bb5e4f9869e777179f602bf5208a4d5524c95b93f235d8`.

Process/start identities were unchanged for bridge 97420, Minime agent 96802,
engine 35303, Division gateway/supervisor 86873/86833, coupled provider 3893,
microphone 3879, camera 3914, visual 3898, host sensory 94952 and feeder 3878.
The host-sensory process had already started at 06:05:52 local time before this
pass; it must not be confused with a restart caused by cleanup.

Post-cleanup checks also verified Minime's shared-helper selection still resolves
to the current immutable stage, engine health advances, and bridge telemetry is
fresh. Controller status remains paused at generation 519 with no lease;
indexed-tail Evidence Event Store verification passes and V1 remains immutable.

Runtime stores, private drafts, source witnesses, journals, qualification logs,
stage inventories and rollback assets were not cleanup targets. Ongoing authored
or runtime writes are allowed; preservation does not assert their contents froze.
No private prose was opened for this maintenance work.

## Preventing Recurrence

`scripts/build_cache_audit.py` is a read-only, explicit-root audit. It recognizes
host, cross-target and trybuild debug layouts, excludes release profiles, and
reports allocated bytes plus free space. Its defaults request review above
**40 GiB debug/test cache per target** or below **50 GiB free**. With `--check`,
budget warnings exit 2; incomplete or unsupported measurements exit 3. Neither
a successful check nor a warning authorizes deletion. APFS sharing means these
figures are not promises of reclaimable physical bytes.

```sh
python3 -B scripts/build_cache_audit.py --check \
  --target-dir /Users/v/other/astrid/target \
  --target-dir /Users/v/other/astrid/capsules/spectral-bridge/target \
  --target-dir /Users/v/other/minime/minime/target
```

`AGENTS.md` now requires this check before substantial builds, explicit review
when thresholds are exceeded, compatible owned-cache reuse, reduced debug and
incremental output where qualification permits, and reviewed closeout cleanup.
These are cooperative build rules, not an OS quota or background pruning job.
Every actual external/worktree target must be supplied explicitly; the default
audit does not claim to inventory the whole machine. Historical evidence and
rollback copies are not evicted because they are old or large.

## Verification and Remaining Scope

- Eleven synthetic audit tests pass: host/cross/trybuild discovery, release/source
  preservation, thresholds, deduplication, missing roots without creation,
  invalid tags, symlink refusal, nested-root refusal, incomplete/failed reads,
  CLI exit status and nonfinite limits.
- The three cleaned parent targets report `within_budget`; only the intentionally
  retained archival debug cache is counted. No Rust rebuild was needed for this
  Python/documentation-only change.
- No edits to Minime source, no Git staging/commit/push in this maintenance pass.
  Astrid's new audit, tests, policy and this record remain an explicit local change.
- The roughly 690 GiB worktree inventory from the preceding audit still needs
  per-owner and per-release review before further cleanup. Model stores, research
  backups, NEAR node data and live databases are separate decisions, not implicit
  cleanup authority.

## Second Pass: Older Worktrees and Restore Tests

Mike then explicitly authorized older worktree caches and verified-safe restore
test copies, while preserving unfinished work, live releases, rollback assets and
original backups. This pass did not remove any Git worktree, branch, authored
history, original backup, release profile or immutable deployment stage.

Local receipts and the exact one-time drivers are retained under:

`/Users/v/other/astrid/.runtime/cache-housekeeping/2026-10-14-second-pass/`

### Worktree Caches

The seven exact target roots were:

| Target root | Preserved source HEAD |
| --- | --- |
| `/Users/v/other/worktrees/astrid-cpu-edge-self-evolution/target` | `8e61f75e8e42203fe854da632f6263b725e93e87` |
| `/Users/v/other/worktrees/study-source-context-20260915/astrid-format/capsules/spectral-bridge/target` | `dd35cdd83a863da373c63e21788a0197768e1175` |
| `/Users/v/other/worktrees/study-source-context-20260915/astrid/capsules/spectral-bridge/target` | `76c2aa43cfd8c9dad084c8f98ef21f2ece6ffaab` |
| `/Users/v/other/worktrees/expressive-context-20260923/astrid/capsules/spectral-bridge/target` | `2f26028b151accf9c841e00d780ce1523c974b13` |
| `/Users/v/other/worktrees/lifecycle-evidence-20260924/astrid/capsules/spectral-bridge/target` | `af20a1fbd21f2287d82e80f1b61cb1b1b604b6b0` |
| `/Users/v/.codex/worktrees/source-selection-recovery/astrid/capsules/spectral-bridge/target` | `c1efa3e995d6b945033faf95b17504966adee7df` |
| `/Users/v/.codex/worktrees/minime-study-exit/astrid/capsules/spectral-bridge/target` | `22da70297be923293b30a5e42643df32d0dd2c32` |

For each root, only `debug` was selected initially. The follow-up audit found
13 additional nested caches in those same roots: `x86_64-unknown-linux-gnu/debug`
under the first root, and `tests/trybuild/debug` plus
`tests/trybuild/aarch64-apple-darwin/debug` under each of the other six roots.
All 20 exact debug scopes were cleared with the dev-profile Cargo command above.
Verbose dry-run path lists were checked to contain only the respective selected
debug directory and descendants. Generated trybuild projects, manifests, sources
and other target contents remain.

The initial seven scopes measured 123.64 GiB allocated; the nested scopes added
roughly 36 GiB. These are cache measurements, not physical reclamation promises.
`cache-before.json`, `cache-after.json`, `cache-final.json`, the per-operation
logs/receipts and `final-cache-audit.json` retain the exact boundaries. The final
read-only audit finds **zero debug-cache bytes in these seven target roots**.

Before every cleanup, process arguments, open files/current directories and
source stability were checked. Installed launch agents did not reference these
worktrees. Every tracked and nonignored untracked file was hashed, and the source
inventory digest, HEAD and Git status matched after cleanup. In particular:

- `astrid-cpu-edge-self-evolution` has unmerged commits; its checkout and history
  remain, rather than being mistaken for disposable work because it is clean.
- `lifecycle-evidence-20260924/astrid` has 19 modified and three untracked files;
  all are byte-preserved. Its dirty source was not staged or rewritten.
- Current candidate worktrees, capacity/provider work, current/rollback stages,
  and the other agent's `tauri-checkpoint` build were outside the deletion scope.

One guard deliberately stopped the nested-cache dry run: Cargo created seven
352-byte `.rustc_info.json` files beside the cross-target caches even with
`--dry-run`. Recomputing the retained-file inventory with exactly those seven
additions omitted matched the original digest. Their JSON identified compiler
metadata; no pre-existing retained content changed. The refusal and reconciliation
remain in `nested-dry-run-reconciled.json` and the logs. Actual cleanup then passed
the stricter retained-file check against the reconciled inventory. A Cargo dry
run is not a promise of absolutely zero incidental metadata writes.

### Verified Restore Duplicates

All paths below are relative to
`/Users/v/Library/Application Support/Reservoir Research`.
The original archive verifier checked the complete backup contents and manifests.
An independent full inventory of each restore checked every file SHA-256, size,
entry kind, archive mode and symlink target, with no exclusions or ignored extras.
Metadata inventories before and after hashing detected changes during inspection.

| Removed duplicate under `RestoreChecks/` | Preserved original under `Backups/` |
| --- | --- |
| `2026-10-06-research-closeout` | `2026-10-06-research-closeout/tree` |
| `2026-10-07-local-computer-backup` | `2026-10-07-local-computer-backup/tree` |
| `2026-10-07-final-integration-v2/repository` | `2026-10-07-final-integration-v2/repository/tree` |
| `2026-10-07-final-integration-v2/Reservoir Scope.app` | `2026-10-07-final-integration-v2/candidate/tree` |
| `2026-10-07-final-integration-v3/final-receipts` | `2026-10-07-final-integration-v2/final-receipts/tree` |
| `2026-10-07-main-integration/receipts` | `2026-10-07-main-integration/receipts/tree` |

These six trees matched exactly. Backup and restore inventories were rechecked
immediately before deletion, with fresh process/open-file checks. No original
backup was a deletion target. The complete `Backups` metadata inventory, including
inodes, sizes, modification/change times and modes, remained unchanged afterward.
The backup verification and restore-removal receipts retain original manifest
hashes and intent/completion records. Historical receipts survive in the verified
originals; their disposable test-copy paths are no longer available.

Two recovery repositories remain unchanged, totaling about **9.22 GiB allocated**:

- `RestoreChecks/2026-10-07-final-integration-v3/repository` (8.88 GiB): the restored
  snapshot was subsequently repaired from source bundles. It has eight extra
  paths and eight changed paths relative to the original v2 archive, including
  Git packs/metadata and later source/documentation changes. The exact differences
  are in `restore-4.json`. Preserved receipts describe reconstruction from the
  original v2 archive plus `source-repair.bundle` and `final-source.bundle`, but
  this pass did not requalify that multi-step recovery. It was not treated as an
  exact duplicate or deleted.
- `RestoreChecks/2026-10-07-main-integration/repository` (0.34 GiB): a source-only
  Git-bundle restore, clean at `ef5a543c66bd6dfd4ab451a22edca6c6eccba8e2`.
  The preserved recovery receipt names `Backups/2026-10-07-main-integration/main.bundle`.
  It lacks a directly matching full-tree archive in this comparison; a fresh
  bundle-based recovery verification is the prerequisite to further cleanup.

Empty historical parent directories were left alone. Original backups total
about 25.71 GiB and are preserved in full. `InquiryRecovery`, application
acceptance/product-check installations, live research applications and other
research evidence were not cleanup targets. No private prose was opened or
included in the maintenance report; content verification used hashes.

### Final Preservation and Status

- Available storage measured 356.42 GiB, up 147.13 GiB from this pass's baseline.
  Concurrent writes and APFS sharing make net filesystem measurements different
  from summed cache sizes or Cargo's logical totals.
- All seven source inventories still match. Both canonical HEADs are unchanged;
  Minime remains clean, and Astrid retains only this housekeeping tranche's
  policy, audit, tests, changelog and steward-note edits.
- Current and rollback bridge stages both verify with the same manifest hashes
  listed above. The selected shared helper still resolves inside the current
  immutable stage and hashes to
  `0ff10d9f11ee2378ae5dc07dac5041b7db3f4e5b426851110dffc5b567ce1ba9`.
- Kernel and engine executable hashes are unchanged. Bridge 97420, Minime agent
  96802 and all protected service PID/start identities match the pre-cleanup
  check. Health and telemetry timestamps advance, with sampled files under two
  seconds old. Evidence: `live-preservation-after.json`.
- All eleven cache-audit tests pass. No Rust source changed or rebuild was needed.
- Controller remains paused at generation 519, without a lease; indexed-tail
  evidence verification is valid and V1 remains immutable. No service restart,
  automation resumption, Git staging, commit, merge or push occurred.

## DaisyDisk Follow-Up: Two Exact Debug Caches

The user separately authorized verification and removal of the 35 GB and 7 GB
debug caches, followed by a model audit without model deletion. This pass removed
only these exact directories:

| Removed debug directory | Allocated KiB before removal | Preserved worktree HEAD |
| --- | ---: | --- |
| `/Users/v/other/worktrees/astrid-cpu-edge-release/target/debug` | 34,427,776 | `8a1f4919b88a7ba58aee8799a3b07ed6dac04283` |
| `/Users/v/other/worktrees/astrid-cpu-edge-self-evolution/services/astrid-edge-runtime/target/debug` | 6,917,724 | `8e61f75e8e42203fe854da632f6263b725e93e87` |

Both clean worktrees and their history remain. Clean status did not make the
self-evolution branch's unmerged commits disposable. Its release and
`x86_64-unknown-linux-gnu` target trees were explicitly excluded and fingerprinted.
The 5,760-file and 6,485-file tracked/nonignored source inventories matched before
and after cleanup. Current and rollback stage verification, native kernel and
engine hashes, Minime agent PID and protected service PID/start identities also
matched. No source, release, model or authored-history deletion occurred.

Commands used `cargo clean --locked --offline --profile dev` with exact manifest
and target paths. Both `--dry-run -vv` listings were checked to contain deletion
paths only within the authorized debug directory before the real command ran.
Process arguments, open worktree files and launch-agent references were checked
again immediately before cleanup.

The first dry run stopped because Cargo refreshed the existing top-level
`.rustc_info.json` in the first target. This was disposable compiler metadata,
not a release artifact; the refusal is retained. A separately named retry
baseline excludes only this specific compiler-cache file from the retained
artifact digest and records its hash separately. It continues to fingerprint
every other retained target file and all tracked/nonignored source. Both dry runs
and real cleanup then passed. No original failure receipt was overwritten.

Available space increased from **356.076 GiB to 395.261 GiB**, a measured net gain
of **39.185 GiB / 42.074 GB**. Cargo's logical totals were 37.1 GiB and 10.4 GiB;
those are not used as the freed-space claim. APFS allocation and concurrent system
activity can differ from logical file totals.

Receipts and the exact one-time driver are under
`.runtime/cache-housekeeping/2026-10-14-daisydisk-pass/`. Successful preservation
receipts are in `verified-retry/before.json`, `dry-run-{0,1}.json`,
`clean-{0,1}.json` and `after.json`; the original dry-run log remains at the parent.
All eleven cache-audit tests pass again. The model audit is documented separately
in `docs/steward-notes/2026-10-14-model-retirement-audit.md`. This pass does not grant
future cache deletion or model retirement authority.

## Git Closeout and Writing Stocktake

Mike subsequently authorized committing and pushing the housekeeping changes,
then taking stock of writing improvements while keeping capacity work separate.
The preceding no-commit statements describe the cleanup passes, not this later
integration. Git history and remote refs identify the resulting checkpoint.

The exact housekeeping set is six paths: `AGENTS.md`, `CHANGELOG.md`, this note,
`docs/steward-notes/2026-10-14-model-retirement-audit.md`,
`scripts/build_cache_audit.py` and `scripts/test_build_cache_audit.py`. One-time
drivers and original receipts stay at their named local evidence paths; model
weights, runtime state and caches are not staged. No Minime source change is part
of this housekeeping commit.

The stabilization claim advances the existing controller pause to generation
**520**, with no active lease. Both component preflights report no foreign
activity. Two canonical status inspections show only these six Astrid paths and
a clean Minime tree. Remote main tips before publication are Astrid `2b36b6d`
and Minime `5c77dd8`; the already integrated writing pair, Astrid `822f50ce` and
Minime `68ae8172`, is one commit ahead in each repository. Those commits contain
the reviewed writing presentation and synthetic adapter qualification, not the
capacity candidate. Publication includes that existing pair without another merge
or restart. The prior automation pause is deliberately not resumed.

Fresh verification on October 14:

- All **11 cache-audit tests pass**. The three canonical target audits report
  `within_budget`; free space is **530,260,828,160 bytes (530.261 GB)** at this
  later sample. This differs from the earlier cleanup readings because the
  machine continues operating. The retained archival debug cache is 3,603,468,288
  bytes; the other two canonical debug caches remain empty.
- Current and rollback immutable stages verify again. All **771 bridge build
  inputs** match canonical main or their retained external dependency. Minime's
  **97 loaded inputs** match canonical main and the qualified snapshot, with
  `reload_required=false`. Its selected helper is the current writing-development
  release and retains SHA-256 `0ff10d9f11ee2378ae5dc07dac5041b7db3f4e5b426851110dffc5b567ce1ba9`.
- Bridge 97420, Minime 96802 and protected process/start identities remain as
  recorded above. Health/telemetry files are fresh. No service was signalled.
- The optional writing regression rerun from canonical Minime returned **27
  passed, 3 failed**. Failures were the reflection-to-private route assertion,
  missing `admitted` in a focus status, and the live-write audit guard interpreting
  a descriptor-relative `.source-status-*` publication against the canonical
  working directory. These are retained as an unsuccessful invocation, not
  hidden by the later pass. Running the identical two test modules with the same
  pinned helper from the clean, previously qualified isolated Minime integration
  checkout passed **all 30 tests in 6.52 seconds**. Both checkouts are at
  `68ae81722079bac0c4470a117a8d6271ed5e6c08`, with unchanged runtime/test files;
  no assertions or isolation guards were weakened. Future qualification should
  retain the isolated-checkout layout. A separate audit-hook portability repair
  may be useful; these results do not establish a live writing regression.

### What Remains for Writing

The latest writing tranche is already live: distinct append/replacement/inspection
framing, exact continuation and branch-return choices, generation-free finishing
and parking, and a clear distinction between ending a passage and finishing a
draft. Full native, shared-reader, immutable-pair and adapter qualification and
the graceful transition are recorded in `2026-10-13-unfinished-writing-development.md`.
There is no remaining native-build or restart gate for that tranche.

The next writing-focused task is a bounded post-release observation of naturally
chosen continuation, stopping, parking and return. Use public entries and
content-free operation evidence unless private inspection is explicitly selected.
Check development across passages, valid NEXT execution and preservation rather
than using word count as a success quota or asking for confirmation of benefit.
The matched historical-cue/reflection comparison remains a separate investigation;
no cue preference or live framing changes are made by this closeout.

The **48,000-byte complete-input allowance and 8,192-token expressive ceiling
remain unchanged**. The larger 96,000/16,384 candidate stays in its isolated
Astrid, Minime and provider worktrees, with all dirty work preserved. The newer
capacity stocktake in `2026-10-13-writing-context-refinement.md` supersedes older
claims that main lacks drain or NEXT recovery. Its remaining work is reconciling
the old maintenance coordinator with current drain-v2 and worker-outcome semantics,
completing mandatory-write/restore and immutable-provider qualification, and
then qualifying resources and the isolated real-model four-cell comparison.
The narrow scanner overlay is tested but not integrated; it is not idle proof.
Reclaimed disk space does not establish RAM headroom or authorize production
stopping. No larger limit, provider transition or model experiment is included.
