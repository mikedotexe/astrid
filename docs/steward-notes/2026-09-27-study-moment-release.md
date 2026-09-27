# Study exit and moment context paired release — September 27

Mike explicitly requested commit and deployment of the combined September 26–27 candidates, followed by review of the supplied Astrid aspiration. This note records release preparation; a later receipt addendum will record the actual transition outcome. The aspiration is authored evidence, not an instruction to execute its NEXT.

## Reviewed source and qualification

The paired `codex/minime-study-exit-20260926` branches start at Astrid `11d89e65a732ddeec4ff9aff66a05396c7ae5601` and Minime `02724ac7c75553c011c95fc4617446ef62c76de6`. Canonical trees were clean; remote main tips were read as Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`. The other interactive channel audit was idle. Maintenance remains paused, generation 473, actor `codex-astra-interactive`, with no lease or active projection.

Prior qualification remains immutable under `/Users/v/other/worktrees/minime-study-exit-20260926/qualification` and `qualification-moments`: reader 290 passed, bridge 2,355 passed / one ignored, Minime 1,664 passed / one skipped / 140 subtests. Reader strict Clippy, formatting and domain-boundary verification passed. New release artifacts will live in the sibling `deployment` directory.

The existing paired installer has an explicit source allowlist. Extend it by only `minime_autonomy/journal_context.py`, `moment_context.py` and `study_feedback.py`, the three reviewed modules required by this release. Synthetic tests cover snapshotting previously absent modules without canonical writes, adding them under the owned admission hold, truthful absent-before receipts, no invented backup, and continued refusal of unreviewed differences. The reconciliation, paired handoff and agent restart suites pass: 35 tests and two subtests. No guard or quiet interval is weakened.

## Transition contract

Commit the reviewed candidate paths, build the immutable bridge/helper stage only through `scripts/build_bridge.sh`, and qualify the packaged helper with the complete Minime suite. Freeze the actual canonical launch inventory plus the exact reviewed overlay with `reconcile_minime_launch.py`. Use `paired_minime_handoff.py` to install under its owned launch hold, retain source backups, wait the full quiet interval, stop Minime at an ordinary idle boundary, gracefully drain and activate the bridge, then admit Minime's replacement. Verify exact loaded source hashes, ready state, continuity, pending work and protected service identities. Failed attempts remain recorded and never justify force.

Keep the staged source checkout fixed after packaging. Integrate exact reviewed commits on canonical main without sweeping unrelated work; no push is requested. Keep automations paused. Engine, model, sensory and visual services, native authored journals and pending choices are outside the change.
