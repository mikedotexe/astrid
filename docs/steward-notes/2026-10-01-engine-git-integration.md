# Engine Integration and Restart Handoff

Later approved outcome: [release-04 is now live](2026-10-01-engine-live-transition.md).
The preparation status below remains the historical record before that approval.

## Result

October 1, 2026, Codex interactive collaborator. Mike requested progress toward
a graceful restart, commit, merge and push. The reviewed engine and release
tooling are integrated and pushed to both `origin/main` branches:

- Astrid implementation `be4658ebff`, integration `63d1f0628fd30a6a4eb61f4d0427c0bdd0171304`.
- Minime implementation `c94db1b`, integration `c428fcaec55090a29b9c3f38c4219b093b1d9172`.

Both canonical trees and indexes were clean after the push. The original
candidate commits and all failed numerical/build/launch-fixture attempts remain
in history. No force push, history rewrite, broad staging or foreign-path cleanup
was used. Current main was merged into the isolated candidates first; documentation
conflicts retain both historical candidate and newer live qualification accounts.
The final bridge/kernel/reader and Minime Python sources are byte-identical to
their previously live main versions. Engine source inputs remain identical to
the prior offline-qualified candidate. Exact changed paths, remote tips, release
identities and process evidence are in [the machine-readable receipt](2026-10-01-engine-integration.json).

**The new engine is not running.** The pending operator question concerns the
old engine/gateway's lack of an acknowledged input drain. Approval has not arrived
in this pass. No service received a signal, no launch hold was created, no engine
binary was installed and no signed deployment handoff was prepared. Do not call
this a completed graceful rollout or claim complete in-flight-input preservation.

## Final Offline Release

Release directory:
`/Users/v/other/worktrees/engine-qualification-20261001/release-04`.

- Source: clean Minime `c428fcaec55090a29b9c3f38c4219b093b1d9172`.
- Manifest SHA-256: `47396e48a469598480dce61ddfc12808682e698b75cbc5eccb845743f25dddbe`.
- Engine SHA-256: `0610800dd57bf34287bb5e4f9869e777179f602bf5208a4d5524c95b93f235d8`.
- Inspector SHA-256: `3f329e59e94458d8160b58384892348c25d1c92a77e0587ffd0521e1ea86d86b`.
- All 120 archived inputs match the canonical engine source and release-03.
- Pinned lock SHA-256: `b7550430a7761fe1cf48f31c0034093c3372c86b8ed71aa852fb69760a83152b`.

The build is locked/offline and read-only, tamper-evident staging, not hermetic or
WORM storage. Binary identities differ from release-03 because the compiled
source identity now names the integrated commit. Later documentation-only commits
do not change this build identity. Never relabel these bytes as a later build.

The actual release-04 inspector passes its eight production-decoder tests under
OS network/write denial. The earlier frozen startup inputs also pass, yielding
PI-only restore and a new unprimed rate clock. Those are copied-input checks,
not the still-running engine's final stopped checkpoint. Covariance checkpoint
lineage remains disabled by the unchanged profile; no full reservoir-state
continuity is claimed.

## Qualification

- 833 selected Rust tests pass again after reconciliation: 427 library, 378 engine,
  seven coupled-harness, two timing-replay and 19 controller-review tests.
- 148 release/support tests pass again with the release-04 inspector, including
  transition, staging, binding, wrapper, controller, evidence and projector tests.
- Selected strict all-features Clippy, formatting and domain-boundary verification
  pass; the boundary audit reports zero violations.
- The canonical sanctioned wrapper's final `--check-only` preflight passed with
  release-04 and the aligned live binding; `activation_performed=false`.
- Raw test transcripts: paired Minime `2026-10-01-engine-integrated-tests.txt`
  and Astrid `2026-10-01-engine-integrated-support-tests.txt`. Verbatim Rust test
  logs retain their blank final line; the explicit `-blank-at-eof` whitespace
  exception is limited to evidence formatting, not source errors.

The existing retained estimator-delay and restored-history high excursions are
not erased by these passing checks. This is not general stability certification.
The narrow transition tests are not exhaustive process-crash recovery tests.
All preceding limits in [transition qualification](2026-10-01-engine-transition-qualification.md)
and [measurement-basis qualification](2026-09-30-measurement-basis-repair.md) remain.

## Launch-Source Alignment Without Restart

Merging three inert owner-hold additions changes launcher files on disk. Leaving
their historical hashes in the current runtime manifest would make truthful
runtime binding fail. Before merging, preserve the exact old manifest and launchers
at `capsules/spectral-bridge/workspace/deployment_manifests/minime-launcher-alignment.20261001/`.

The old manifest hash is
`49acf3a94fd3cc3642d854781c0fbbbd0998f8ccfccbb59b9cbf620c80275578`.
After merging, verify the launchers differ only by the reviewed wait block, with
no hold present. Verify all engine/companion and protected PID/start tuples and
the mapped Mach-O identities remain unchanged. Update only current launcher
artifact references plus an explicit `launch_source_alignment_v1` explanation;
preserve the historical engine hash, build time and repository provenance.

The aligned current manifest hash is
`15ba328d830747913bba5c6beba1d490b5f3a5393b7fc12c9d88c10bf041d79c`.
The original bytes remain archived. A new mapped-build binding succeeds at
`deployment_manifests/minime-runtime-binding.0fsee8r4/runtime-binding.json`,
SHA-256 `7497c4b8379029292161109df30a7b49105a849dd27fce03ec1a8fc8d4cde291`.
This binding expires after 180 seconds and must be recaptured before activation.

Running engine 3906, gateway 3887 and supervisor 3897 retain their September 30
14:29:07 process starts. Installed engine SHA-256 remains
`0e40bcf7ed944f38a65c45ee85b4916484c6938bc8bb665dff53646d762821d6`.
Bridge, agent, model, visual, microphone, camera and host-sensory processes remain
unchanged. The dormant Division routing/authority and live preferences were not
modified. Launcher alignment is not candidate-engine activation.

## Next Transition Boundary

1. Obtain the pending, specific one-time transition approval. The proposal permits
   bounded SIGTERM only for the identified old engine, gateway and supervisor;
   it does not promise lossless in-flight input or permit forced termination.
2. Recheck paused controller/no lease, foreign activity, source/stage hashes,
   current profile, protected identities and dormant Division. Recapture the
   binding with `python3 scripts/minime_runtime_binding.py`.
3. Use only `bash scripts/deploy_minime.sh --activate-stage` with release-04,
   its exact manifest hash, a fresh binding and a new transaction directory.
   Supply the actual approved acknowledgement, not the check-only placeholder.
   The launcher source remains the paired Minime worktree above.
4. The wrapper must verify old exits, a post-signal stopped checkpoint and exact
   signed state lineage before release, then verify mapped binaries, readiness
   and at least 360 seconds of bounded observation. Any refusal remains a refusal.
   Recovery uses newest state, never a historical preference backup.
5. Archive actual activation or recovery receipts and verify remote/source/live
   distinctions again. Observe natural use without requesting endorsement.

Automations remain paused at generation 485. Evidence verification reports V2
sequence 1123139, head
`3e305b63ff7ec974a3de78c8704190cecc3cf636607997c42692e1a718d78e51`,
with V1 immutable. This is interactive release work, not an introspection round
or a claim that the source queue is current. No private writing was inspected or
rewritten. The original public witness and its SHA remain in the paired notes.
