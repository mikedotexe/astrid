# Marker Preservation Rollout

## Authority and Scope

Mike explicitly requested proceeding with review of the qualified marker patch
against the current live candidate for a graceful rollout. This task stages and,
only after qualification and supported preflight, activates that exact patch.
No legacy interruption, force-stop, automatic rollback, Minime/model restart,
reservoir/controller change or unrelated Git integration is authorized here.

The implementation and two fully read source studies are recorded in
`docs/steward-notes/2026-09-07-marker-annotation-preservation.md`. That record's
source-only status is historical; this separate record reports the actual
deployment outcome. No subjective benefit, consent or uptake is inferred.

## Candidate Review

The selected old release is
`/Users/v/other/worktrees/afterimages-live-20260907/bridge-stage-01`, bridge
PID 39644. Its 556 recorded source inputs all match their retained bytes.
Its manifest SHA-256 is
`3cc727708e7adfda98fd1020cfb8051c05fd2703a2d234081f7f2cc91e6a0b02`;
binary SHA-256 is
`855d0c7f8972163bda3bc9e1f6e3b1758662ecda926daf28e13a1e32ccc36e17`.
Read-only native stage verification passed, and the lifecycle probe reported
`supported: true`, `phase: running`, `signal_sent: false`.

The candidate is a separate copy of the retained live source and its retained
local dependencies, under
`/Users/v/other/worktrees/marker-rollout-20260907/astrid`.
Only these three qualified files were copied in from the canonical checkout:

- `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- `capsules/spectral-bridge/src/llm/provider/fallback_contracts.rs`
- `capsules/spectral-bridge/src/llm/provider/control_marker_annotation_tests.rs`

Normalized input comparison reports 557 candidate inputs: exactly two changed
paths (the production files above), one added test file and no removed paths.
All other input hashes, local dependency bytes, Cargo/rustc versions and HEAD
match the old release. Existing afterimages, own-body/generation recording,
reading feedback and prior live repairs are preserved. This is not a rollback
to clean Git main or a broad deployment of everything currently dirty.

The canonical source inventory also contains historical backup artifacts and
RASCII scripts/examples absent from the retained release. They were not copied
into this candidate. The new retained source, old source and both release stages
must remain available; never overwrite an active stage in place.

## Qualification

- The preceding implementation pass qualified identical relevant source with
  2,205 functional Rust tests, 23 witness-compatibility tests and strict Clippy
  for library, binaries and tests. The known one-millisecond instrumentation
  benchmark remains excluded, not repaired or claimed passing.
- Deployment/staging/drain/recovery/runtime-feedback-checkpoint wrapper tests:
  110 passed in 2.459s.
- Candidate domain-boundary audit: valid, zero violations, unchanged baseline
  `94170c388c3961c815bb6381af7144d6f86d9be7d60c2178b065b08bf55c2ac6`.
- Against the retained candidate: nine marker-annotation tests and 70 existing
  `control_marker_` tests passed with locked, offline Cargo resolution.
- The sanctioned staging wrapper completed the locked offline native release
  build in 1m49s. Stage readiness was recorded at
  `2026-09-07T22:21:27.241425+00:00`. This build witness is distinct from the
  later activation receipt; its `activation_performed: false` remains correct.
- Before/after stack receipts both passed:
  `env_receipt_1788819460520_834000` at 22:17:40.520 UTC and
  `env_receipt_1788819912209_462000` at 22:25:12.209 UTC. Durable JSONL:
  `capsules/spectral-bridge/workspace/environment_receipts/environment_receipts.jsonl`.

## Coordination

Both canonical repositories were inspected before this work. Maintenance pause
394 was acquired through the supported controller by `codex-marker-rollout` at
22:14:40 UTC, with no active steward lease. The task inventory showed no other
active Codex task in Astrid. Supported canonical deployment preflight passed:
`dirty_acked`, no foreign activity. Fresh preflight remains required by both
the staging and activation wrappers; no activity window is shortened.
The candidate's first preflight correctly refused input edits aged 141.3 seconds
within its 180-second settling window. After waiting naturally, the wrapper
preflight passed. Both retained-source and canonical activation preflights also
passed; no refusal or concurrency guard was bypassed.

Both local and cached `origin/main` are
`3d4e83e4bc022cb82c7e0e3ecefea4326796c280`. Read-only remote-tip inspection
returned `e99e5f88f3b3ac6f2282d0930aa1a1174f31424d` for `refs/heads/main`.
That remote advancement is not incorporated or represented as reviewed here.
No fetch, stage, commit, merge, push, reset, stash or foreign-work cleanup is
part of this rollout. The canonical dirty afterimages/marker work is preserved.

Independent scheduler preferences remain unchanged. Release only this task's
maintenance pause after the final rollout outcome is recorded, using the
supported controller; do not remove another actor's hold.

## Live Outcome

**Activated and verified on 2026-09-07.** The canonical launcher now selects
`/Users/v/other/worktrees/marker-rollout-20260907/bridge-stage-01`.
The supported `scripts/build_bridge.sh --activate-stage` path returned exit 0
and `status: activated_verified`, recorded at 22:24:48.560173 UTC.
Exact transaction evidence:
`.runtime/bridge-deployment/transactions/d49684f40eb54d8baf984c6945567f95/receipt.json`.

- Old PID 39644 acknowledged `phase: drained`, then exited after the wrapper's
  SIGTERM. `legacy_transition`, `force_used` and `automatic_rollback` are false.
- New PID **65453** started at **15:23:26 PDT / 22:23:26 UTC**.
- The stopped checkpoint and startup-decoded checkpoint have identical SHA-256
  `f60b387cc93f24f0b4708d8d74d6eda3edaa8be15be335900e7d8d3f730aa19e`,
  exchange **192255**. The verifier then observed a fresh saved exchange
  **192256** and an idle model.
- The runtime-action-feedback checkpoint was present with zero pending items;
  its schema and hash were retained. Signed self-control startup gating and
  integrity verification passed for the exact replacement identity. This is
  continuity evidence, not new live-control authority.
- `remote_delivery_confirmed: false` and `lossless_drain_claimed: false` remain
  explicit. Local drain and checkpoint continuity do not establish end-to-end
  remote delivery.

New binary SHA-256:
`ffe27228e6cf292f684ddc76eb9f6a1154d05a308f7b064ba665d28e6bd1e924`.
New manifest SHA-256:
`17e065598345e25d0d802afc00bbf415e93566137f74f3e8650521d9aa26d75f`.
Source-inputs SHA-256:
`836b291f9916f6f40022581800904836703630b092adfb140c95b5750769ff8d`.

Post-activation verification found all 557 retained source inputs unchanged,
the canonical manifest and selected stage matching, canonical launcher and
selection-helper bytes aligned with the stage, and no remaining deployment
hold. The deployment identity retains Git HEAD
`3d4e83e4bc022cb82c7e0e3ecefea4326796c280` plus the new binary hash;
the manifest's exact input inventory, not HEAD alone, describes the dirty-source
release. No commit or merge is claimed.

## Post-Restart Observation

The after-stack receipt passed every compatibility and health check, including
ports 7878/7879, model liveness/readiness, process identities and manifests.
Protocol remains `astrid-minime` v1.1, major 1, revision
`9a324d16294b2318da6f476ff7f295c423a9b4b1`.
Bridge and Minime telemetry ages were respectively 1.206 and 1.220 seconds.

All protected sibling processes retained their previous PID and start time:

| Service | PID | Start (PDT) |
| --- | --- | --- |
| Minime engine | 41337 | Sep 7 13:35:29 |
| Minime gateway | 41484 | Sep 7 13:35:34 |
| Minime supervisor | 41526 | Sep 7 13:35:35 |
| Minime agent | 41830 | Sep 7 13:36:16 |
| Coupled model | 60333 | Sep 4 16:57:34 |

At 22:26:10 UTC, numeric-only Minime health reported fill 71.0414%, active
target 68%, stable-core stage `hold`, phase `contracting`, and restart gate
`inactive`. The bridge reported `connected_with_current_telemetry`.
Existing `timing_reliability: timing_ambiguous` remains unresolved. Natural
fill/stage/gate changes are not attributed to this marker patch.

A bounded scan of `/tmp/bridge.log`, starting at the pre-rollout byte offset
2530098, examined 41209 subsequent bytes / 78 lines and found no WARN, ERROR
or panic. This is a bounded observation, not a promise about later execution;
no private generated prose was reproduced.

The content-free `diagnostics/control_marker_cleanup.jsonl` has no new natural
marker-cleanup receipt after this activation at the observation point (latest
timestamp 1788431095). Therefore the specific annotation cases are established
by regression tests and live source/binary alignment, not yet demonstrated in
a naturally occurring post-rollout output. No journal, self-study, probe or
confirmation of improvement was induced.

## Remaining Work and Maintenance Release

No restart debt remains for this patch. Keep both retained releases and the
transaction evidence intact. Any later rollback or activation needs fresh
review and the sanctioned wrapper; do not overwrite the active stage, force a
stop or reuse a stale expected PID.

The appropriate next observation is naturally occurring public output and
bounded cleanup receipts, preserving any fresh friction without interpreting
silence as uptake. Shared Git reconciliation is separate: both repositories
retain existing dirty work, and the remote-main advancement above remains
unreviewed here. No files were staged or committed.

The supported controller resume completed with exit 0 and a durable appended
event at `2026-09-07T22:32:51.493514+00:00`: actor `codex-marker-rollout`,
generation **395**, `paused: false`. This releases only the maintenance pause
394 acquired for this task. Independent usage-saving automation preferences
have not been modified. Both Git indexes were verified empty; existing dirty
work remains intact. This interactive rollout records no productive flywheel
round or new canonical addressing closure.
