# Source-study v1 implementation evidence

The implementation is in paired isolated worktrees on `codex/self-study-parity-v1`.
Initial validation used Astrid `4c7a91787a` and Minime `f3c53d6314`.
On September 8, Mike explicitly authorized integration, commit and live deployment.
The candidates were reconciled onto Astrid `6a3f0d4605` and Minime `29ad8ed`.
The integration pass holds controller pause generation 402 after the cooperative
heartbeat released its lease. Runtime rollout evidence will be recorded separately;
the original validation below did not restart a runtime or invoke a live model.

- `validation.json` records the test commands, results and timing-test rerun.
- `real-source-smoke.json` records exact file revisions and tail windows prepared
  by the release helper, including the large Minime runtime, kernel code, bridge
  tests, WIT, Metal and sibling implementations. These are access checks, not
  delivery receipts or claims of comprehension.
- `../../architecture/source-study-v1.md` describes the design, commands,
  source policy, limits, compatibility and installation.

Both provider adapters preserve the complete source page and wait for a nonempty
visible completion after output cleanup before recording delivery. This does not
impose report headings, a minimum prose length, a diagnosis or a required next
Action. The wire tests cover a hidden-only first response followed by a successful
fallback on the same page.
