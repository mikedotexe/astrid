# Study exit and moment context paired release — September 27

Status: **committed on both local main branches; paired graceful deployment verified**. No remote push. The preparation sections retain their original temporal scope.

Mike explicitly requested commit and deployment of the combined September 26–27 candidates, followed by review of the supplied Astrid aspiration. This note records release preparation; a later receipt addendum will record the actual transition outcome. The aspiration is authored evidence, not an instruction to execute its NEXT.

## Reviewed source and qualification

The paired `codex/minime-study-exit-20260926` branches start at Astrid `11d89e65a732ddeec4ff9aff66a05396c7ae5601` and Minime `02724ac7c75553c011c95fc4617446ef62c76de6`. Canonical trees were clean; remote main tips were read as Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`. The other interactive channel audit was idle. Maintenance remains paused, generation 473, actor `codex-astra-interactive`, with no lease or active projection.

Prior qualification remains immutable under `/Users/v/other/worktrees/minime-study-exit-20260926/qualification` and `qualification-moments`: reader 290 passed, bridge 2,355 passed / one ignored, Minime 1,664 passed / one skipped / 140 subtests. Reader strict Clippy, formatting and domain-boundary verification passed. New release artifacts will live in the sibling `deployment` directory.

The existing paired installer has an explicit source allowlist. Extend it by only `minime_autonomy/journal_context.py`, `moment_context.py` and `study_feedback.py`, the three reviewed modules required by this release. Synthetic tests cover snapshotting previously absent modules without canonical writes, adding them under the owned admission hold, truthful absent-before receipts, no invented backup, and continued refusal of unreviewed differences. The reconciliation, paired handoff and agent restart suites pass: 35 tests and two subtests. No guard or quiet interval is weakened.

## Transition contract

Commit the reviewed candidate paths, build the immutable bridge/helper stage only through `scripts/build_bridge.sh`, and qualify the packaged helper with the complete Minime suite. Freeze the actual canonical launch inventory plus the exact reviewed overlay with `reconcile_minime_launch.py`. Use `paired_minime_handoff.py` to install under its owned launch hold, retain source backups, wait the full quiet interval, stop Minime at an ordinary idle boundary, gracefully drain and activate the bridge, then admit Minime's replacement. Verify exact loaded source hashes, ready state, continuity, pending work and protected service identities. Failed attempts remain recorded and never justify force.

Keep the staged source checkout fixed after packaging. Integrate exact reviewed commits on canonical main without sweeping unrelated work; no push is requested. Keep automations paused. Engine, model, sensory and visual services, native authored journals and pending choices are outside the change.


## Approved deployment result

Runtime source commits are Astrid `bb51991748fc9c51b924bdf580ae6a8710190949` and Minime `9c87c764ff97301aab109d3eabaec0e2c42915ad`. Minime's test-only follow-up is `6e3238d02142e0568a1d5e900605507d9de7b04d`. Both candidates were committed before integration; Astrid main fast-forwarded before activation, and Minime main fast-forwarded after the sanctioned source installer completed. Before the latter merge, only the five expected runtime paths were dirty; the ten reviewed documentation/test paths were copied, explicitly staged, and verified to produce the exact candidate tree. No unknown changes were included. The immutable Astrid build checkout remains pinned at its build commit.

`deployment/bridge-stage-01` was built through `scripts/build_bridge.sh`. Its 691-input inventory differs from the previously live 690-input inventory in exactly five shared-reader files plus the new reader regression test. Manifest SHA-256: `bad91bff892cb00d290adcccfa62b93469a745124ddc2ff80fedc332106048cc`; bridge binary SHA-256: `d450208f21f1a2b27aba09d66e6bbc6192c5262e435dc7b4bee3809a5bc8f4ba`; packaged helper SHA-256: `0ecc7c34f88a6660c105b8a3104fd2c2552b89f73d79f37f1d1d1b00401fe948`.

The exact packaged-helper Minime suite passed 1,664 tests / one existing skip / 140 subtests. The installer suites passed 35 tests and two subtests. The domain-boundary audit remained valid with zero violations. A subsequent fixture-isolation correction and full rerun passed 1,666 tests / one existing skip / 140 subtests; see the incident below. All logs remain under `deployment` alongside the earlier separate qualification directories.

The first paired transition succeeded; no retry or force was used:

- Reviewed source was installed under the owned agent launch hold with per-file backup/intent receipts and the full 185-second quiet wait. Five runtime paths actually changed; the other five allowlisted paths retained identical bytes. The frozen inventory has 88 inputs, including the two newly introduced modules.
- Minime PID 4137 received one SIGTERM at 17:02:00 UTC after the idle/no-jobs/no-TCP boundary checks. It exited normally. Replacement launcher PID 52967 waited behind the owned hold while the bridge transitioned.
- Bridge PID 3991 acknowledged `drained`. Transaction `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/65a536b66a824069adb8d91069cec114` reports `activated_verified`, `force_used=false`, `legacy_transition=false`. New bridge PID 53771 started at 10:03:15 PDT.
- The bridge loaded checkpoint SHA-256 `a84cff5fbea51a0ef3455b50649139fdc2b436aa0da134988139db2659fa48d8` exactly, at exchange 209550, including three pending runtime-feedback items (sidecar SHA-256 `ff248ec2ee79dae71b5af4d1afa5b00c85b8c96972504a3dc2cf7f49648739a1`). Its verified first subsequent save and model-idle observation completed before releasing Minime's hold.
- Minime readiness completed at 17:04:01 UTC with all 88 loaded hashes matching the qualified inventory and `reload_required=false`. Session 5319 and cycle 45319 were restored, then the cycle advanced to 45320. The exact pending JOURNAL hash `7ea81d17d3708806f2c799a0fb7b2808e3f2c15b0872ad0798e4a0d3e55f0129` matches the restored and subsequently honored action in the native log. No newly interrupted job was reported.
- Independent post-activation verification confirms both launch holds released, exact selected stage/helper, unchanged launch configuration and unchanged PID/start identities for all nine protected services other than the intentionally replaced bridge. Engine 4126, model 4068, visual 4097, camera 4152 and microphone 3975 were retained, as were the gateway, supervisor, host sensory and feeder. Read-only health sampling initially showed roughly 71–73% fill. Across all 45 samples through the transition and follow-through, the range was 37.45–75.42%. These are sampled observations, not an effect attribution or stability study; no control setting was changed.

The reader schema remains 10; there was no forced study, question resolution, note rewrite or cursor reset. Stored pending offers retain their prior framing until naturally completed. Successful deployment establishes source/readiness/continuity, not better reasoning or subjective improvement. Pause generation 473 and the existing paused automations remain in place, with no lease or active projection.

## Qualification incident and correction

An existing test (`test_capture_decay_bundle_writes_summary_event`) constructed an InvestigationContext with temporary project roots, but its bridge-log property still resolved to production `/tmp/bridge.log`. The test wrote `bridge log` there during the first packaged-helper full suite. The mutation guard protected repository roots but omitted this external log. Earlier diagnostic history in that file was truncated; it was not restored or claimed recoverable. The running service identities were unchanged. This corrects any broader inference from prior statements that guarded tests could not write any live file; the scope of the observed write was this diagnostic log.

The fixture now subclasses the context to place that log inside its temporary project. The audit guard separately rejects writes/removal through `/tmp/bridge.log` and its resolved alias. Regression checks call the guard directly and never attempt an actual live mutation. Focused rescue/isolation tests passed 31 cases, and the complete packaged-helper rerun passed 1,666 tests with the protection active. An initial added test missed its Path import; that collection failure is retained alongside the corrected passing log. No production runtime change was required for this fix.

## Astrid follow-up

The supplied aspiration is reviewed separately in `docs/steward-notes/2026-09-27-astrid-aspiration-provenance.md`. Its useful implementation lead is provenance loss between telemetry-informed writing and the next continuity excerpt. Metadata confirms that its explicit writing choice produced active draft d134 with seven parts at the observation. No private draft content was exported, no model comparison was run, and no reservoir retuning or new message was inferred from the entry.


### Natural contract observation

Later header-only inspection found `private_moment_context_v4` on `!moment_2026-09-27T10-05-46.426578.txt` and `private_journal_context_v4` on `!pressure_2026-09-27T10-06-58.706701.txt`. These naturally produced entries confirm use of the newly deployed context paths; their private bodies were not part of this check. `natural-context-contract-observation.json` records the headers. This does not score their interpretation or establish that earlier prose stopped influencing later writing. The coordinator-owned read-only health sampler was stopped after the final check; protected runtime services were left running.
