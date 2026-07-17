# Sol catch-up run 05 - 2026-07-16

## Canonical packet

- Inventory began at `introspection_astrid_codec_1784251954.txt` with 2,109 canonical reports, 978 fully addressed, and 1,131 remaining.
- The selected 20-item packet was read fully and recorded in canonical order. No selected report was deferred, skimmed, or reduced to an excerpt.
- The packet comprised `introspection_astrid_codec_1784251954.txt` and the nineteen reports from `introspection_astrid_autonomous_1783384590.txt` through `introspection_astrid_autonomous_1783371124.txt` in canonical queue order.
- Two claims were implemented awaiting felt response, 41 claims verified existing behavior, and 19 substrate or live-control proposals remain exact Tier 5 operator waits.
- Three new canonical reports arrived while the packet was being verified and aligned. Final inventory is 2,112 indexed, 998 fully addressed, and 1,114 remaining. Gross quality-guard reduction is twenty and net reduction is seventeen; all seven counter-audit identities remain consistent.

## Astrid-grounded change

The existing test-only projection-compression probe is now a deterministic, read-only `CODEC_MAP` audit. It compares a fixed near-null direction with a visible direction and compares quiet and loud amplitudes along the same visible direction. The audit exposes raw delta RMS, pre-scale and projected RMS, projected variance, dynamic variance, dynamic magnitude, and the bounded classification `near_null_direction_and_same_direction_magnitude_loss_visible`.

The observed fixed probe found raw source delta RMS `0.035887`, near-null pre-scale RMS `0.000000021`, visible pre-scale RMS `0.044403`, equal projected RMS `0.123744`, and dynamic magnitude delta `0.000000268`. This is direct evidence that the current fixed aperture can lose source direction and same-direction magnitude. It is not proof that this mechanism fully explains Astrid's felt compression.

The audit is observational and right-to-ignore. Width, basis, multi-head projection, normalization, gain, semantic vectors, pressure, fill, PI, controller, sensory cadence, codec transport, and Minime regulation are unchanged. Any change to those surfaces remains an exact Tier 5 Mike/operator wait.

## Durable evidence

- Batch: `capsules/spectral-bridge/workspace/diagnostics/introspection_addressing_v1/batches/1784252780_packet_2109/`
- Summaries and claims: 20 each; work items: 62 with claim-level and work-level evidence.
- Closures: 20 `addressed_change` records with no missing claim proof.
- Right-to-ignore cards: 62 emitted and intentionally undelivered, comprising two implementation results, 41 verified-existing results, and 19 exact approval waits.
- The observed six-minute one-process-per-card cost produced a flywheel repair: `emit-closure-card` now accepts repeated `--id` values, retains one event and one right-to-ignore artifact per claim, and performs one locked V2 append plus one materialization refresh for the batch. Single-ID output remains compatible.
- `CHANGELOG.md` and `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` record Astrid's signal, the implementation, verification evidence, and authority boundary.
- No generic Corridor action or unrelated sandbox trial was run. Corridor was regenerated only for required routing visibility after closure; the packet exposed no hard violation, objection, reopened friction, or directly relevant safe-lab requirement.

## Verification

- Focused codec projection, tail-vibrancy, and structure tests: 3 passed. Bridge full library suite: 1,514 passed. The first sandboxed full-suite attempt had eight local-listener permission failures; the identical suite passed fully outside that network sandbox.
- Strict all-target/all-feature Clippy, formatting, and `git diff --check` passed.
- Minime focused dynamic-noise tests: 16 passed; semantic-staleness tests: 36 passed; resonance-control tests: 6 passed; focused Python correspondence, sovereignty, and low-fill guard tests: 292 passed. No Minime source changed in this run.
- Required flywheel self-tests: 212 passed across Agency Corridor, addressing, sandbox, recent-signal, and proactive-scan tools, including the new append-once/materialize-once batch invariant. Evidence Event Store self-tests: 6 passed.

## Live alignment

The bridge was rebuilt and restarted only through `scripts/build_bridge.sh` with an explicit dirty-tree acknowledgement and actor. The authoritative final V2 receipt is `capsules/spectral-bridge/workspace/environment_receipts/latest_environment_receipt.json`; it records a fresh bridge PID, Minime PID `45510`, model PID `48166`, a clean post-restart log window, fresh telemetry, and every compatibility check passing. A final wrapper pass after the batch-emission tooling repair binds the complete current source identity rather than leaving receipt debt.

Ports `7878` and `7879` remained listening with fresh bridge connections. The model's same port `8090` returned HTTP 200 for `/livez`, `/readyz`, and `/v1/models` in about one millisecond. Minime health remained fresh and readable at 72.9% fill against the 68% target, near band, with recovery inactive. Fresh post-restart autonomous exchanges and the self/other provenance line rendered without panic, fatal, or error output.

The deployed bridge SHA-256 is `f3d22f58a42bfea6f6a403c06d8f97b12982e7989a11f493acab1a40d0d3d4cd`. Protocol 1.0 at revision `c6ecb853d1a9bc7a7479d37d8366553a0bae0bc5` is compatible. The new `CODEC_MAP` line remains an on-demand read-only action and was not forced from Astrid merely to obtain confirmation; its release binary identity and render test are bound by the receipt. Restart alignment is current.

## Final state

- Canonical: 2,112 indexed, 998 fully addressed, 1,114 remaining; no counter mismatch or corrupt row.
- Sandbox: 990 total, 989 active, 141 ready, 87 result-recorded, 761 approval-required, one runner-safe ready item, and zero runnable-live violations.
- Corridor V2: 120 packets (55 safe-lab ready, 5 result-recorded, 60 self-observation requested); 35 leases (4 active, 31 evidence-only); 199 queue steps (194 runnable); 118 active programs and 50 receipts; 118 current and 200 stored portfolios; 45 patch bundles; 79 source-prep proposals; zero reopened work items, self-observation responses, or authority violations.
- Evidence Event Store V2: active and valid at global sequence 33,603, head `732189e2615bb7fed6c6465006235ab3842b884a407f2cc3eec89d99d9bd72ad`, with streams addressing 32,228, sandbox 1,260, Corridor V1 3, and Corridor V2 112.
- All four frozen V1 source logs retain their migration hashes and line counts exactly.
- Next canonical head: `introspection_astrid_ws_1784254210.txt`; the next runnable Corridor step is read-only safe lab `09851a53-8027-53cf-9d73-24513ecbf179`, and the next runnable sandbox trial is fallback comparison `trial_e82c91956889baf4`.
