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

Pending qualification and integration. Bridge deployment must use
`scripts/build_bridge.sh` staging and acknowledged graceful activation; Minime
uses the existing observed-idle graceful Python-agent reload. Exact loaded source
identities, pending/checkpoint continuity and surrounding services will be checked.
No induced studies, private-writing prompts, or execution of journal instructions
will be used to manufacture uptake. No behavioral improvement is yet established.

Board mirroring remains pending.
