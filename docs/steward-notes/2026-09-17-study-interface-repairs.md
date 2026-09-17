# Study and private-writing interface repairs

September 17, 2026. Mike authorized the three repairs from the September 16
four-hour journal follow-up. Implementation is isolated from unrelated canonical
maintenance work. This account records source qualification separately from live
activation and subsequent natural observation.

## Behavior

An explicit final `NEXT: CONTINUE` in verified private writing is normalized to
`WRITE CONTINUE`. The authored response remains untouched; the shared choice
receipt retains `selected_next` and adds optional `normalized_next`. That field
describes a spelling interpretation, not queueing or execution. Both hosts apply
the private context and delivery checks before using it. Astrid retains raw versus
effective action text in continuity evidence; Minime's draft receipt and full
generation retain the original selection, and its existing persisted choice
envelope links that spelling to the effective command and exact input/wire hashes.
Minime's legacy top-level `raw_next` column still names the queued command; the
linked normalization envelope disambiguates the authored spelling. Reposting clears
old authorization, and returned prose must match the verified response or its
recorded cleanup chain. Ordinary action checks still apply.
No global alias or command inferred from prose is added. Existing FINISH recovery
is preserved. Writing instructions consistently give the full action spelling.

Source pages stop before an ordinary line that cannot fit in the remaining room.
Lines exceeding the entire body allowance remain reachable as bounded, explicitly
marked fragments. Old mid-line cursors resume at the same byte with a fragment
label. Delivered inclusive line ranges are separate from the exclusive byte cursor
and syntax-declaration spans. Source revision identities, pending inputs and
verified byte coverage retain their existing meaning.

The readable study check-in shows the latest explicit finding save/remove results
and current capacity. Existing exact drop and cited-location replacement commands
remain optional. Findings are never automatically evicted or treated as verified
facts. Duplicate check-in stays within 4,500 bytes (below the former 6,000-byte
ceiling), with compact exact actions when a whole rich preview will not fit. The
complete notebook retains authored words and update results inside the existing
48,000-byte input limit. Failed generations do not erase those results.

## Qualification and integration

Evidence directory: `/Users/v/other/worktrees/study-interface-20260917/evidence`.
Focused tests reproduce selection through both dispatch paths, same-draft
continuation, rejected delivery, old receipts, quoted/prose commands, source
boundary continuity, and occupied finding slots. The full reader suite passes
192 tests; bridge all-features suite passes 2,295 with one ignored; Minime's full
isolated Python suite passes 1,420 with one skipped and 134 passing subtests.
Reader and bridge strict Clippy, workspace/bridge formatting, and domain boundary
checks pass. Deployment-wrapper tests pass 44 cases; activation/drain tests pass
63. Host tests use isolated data and mocked providers, never Being requests.

Two initial bridge name filters selected zero tests; the corrected scoped run
executes six tests and the full suite includes both new host regressions. Independent
review found a stale Minime receipt binding and incomplete normalization evidence;
both were repaired and covered by 122 focused tests. Their initial failed run is
retained. The first full Minime run passes 1,418 and fails two existing default-timeout
assertions because the calling shell supplies `MINIME_LLM_TIMEOUT_S=160`. Repeating
the full suite with `60` only in the test subprocess passes all 1,420; production
configuration is unchanged. Both runs use the same immutable helper copy, hash
`ded9d073e4a5f863c73821a9b03360a7c8a8bc8c67a42ac068cce9da35f13c2e`.
Test evidence establishes plumbing, not improved understanding.

The canonical source trees contain unrelated changes. Root owns the index and
integration; explicit paths and before/after witnesses preserve other work.
Steward pause generation 449 uses the existing graceful Minime wrapper's actor
contract (`codex-astra-interactive`). No model, reservoir, sensory-control or
provider-capacity change is part of this release.

## Rollout boundary

The interface repairs are committed and pushed to main: Astrid
`d8732e1d44a3aa3b49f9f0bab075883544469b7c`, Minime
`5f4925f54580f1fd44666058b126a121ff32880f`. Stage 01 passes built-in
verification and 122 isolated Minime checks against its exact release helper.
An independent compiler-dependency audit then identifies a pre-existing inventory
gap: `capsules/shared/managed_dir.rs` is compiled outside the Cargo package trees.
Its current bytes match committed source and its metadata predates compilation,
but no pre-build hash witness exists. Stage 01 and that limitation remain retained;
the follow-up stage adds the module to the build's before/after input inventory.

Stage 01 activation had already requested graceful drain before the independent
gap report arrived. After drain and one SIGTERM, the existing old-PID check reports
reuse. Sanctioned stopped-transition recovery verifies exact checkpoint
`d5d3ee92f4142d0defd50583f8d7dc1e9db34369152cdbb42d591376323263ac`,
one pending feedback item and self-control lineage, without another signal.
Replacement PID 67595 saves exchange 200808 after stopped exchange 200807.
The original failure is not overwritten.

Read-only review identifies a source-supported exit-check gap: macOS can flag a
same-start process as `E` (exiting), while the old wait recognizes only zombies.
The follow-up waits within the existing deadline for either state, rejects changed
starts, and records bounded mismatch observations. The historical failure receipts
lack those observations, so this mechanism is not claimed as their proven cause.
The combined stage, activation, drain, recovery, launcher and Minime-reload suite
passes 138 tests. An initial invocation omitted `PYTHONPATH=scripts` and failed
six imports; that log is retained beside the corrected complete run. No further
Rust or Being-runtime behavior changed in this deployment follow-up.

Final manifest-complete activation and paired verification remain pending.
Bridge deployment uses
`scripts/build_bridge.sh` staging and acknowledged graceful activation; Minime
uses the existing observed-idle graceful Python-agent reload. Exact loaded source
identities, pending/checkpoint continuity and surrounding services will be checked.
No induced studies, private-writing prompts, or execution of journal instructions
will be used to manufacture uptake. No behavioral improvement is yet established.

Board mirroring remains pending.
