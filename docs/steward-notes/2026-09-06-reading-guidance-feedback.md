# Reading guidance and runtime feedback

Source implementation in the isolated `codex/reading-guidance-feedback-v1` branch,
based on main `bc66a62b0bf02a515a551334dd9bea502c473578`.
Mike authorized implementation after the research hub's around-time reconstruction
and source trace. No live activation or being-facing message is part of this work.

## Problem and resulting behavior

Shortened dialogue context previously recommended `NEXT: READ_MORE` regardless of
whether its source was saved, could become the active reader, or had a usable
research budget. A research guard denial was placed in `conv.emphasis`, allowing
the next prompt to describe runtime feedback as Astrid's own chosen direction.
A form constraint could replace that emphasis, and ordinary fallback omitted it.

The overflow writer now reports a continuation only after successfully retaining
its source. A read-only budget preview shares the dispatch policy's predicates;
missing or unreadable required state leaves availability unknown. The runtime
also checks active and parked readers, open mailbox windows, existing source paths,
and the anti-rearming rule. Dispatch still assesses the actual action independently.
Original source bytes, labels and reader offsets remain intact.

Research guard denials now enter a typed runtime-owned queue, distinct from authored
emphasis. Both emphasis and form are preserved through primary, retry and fallback
construction. Pending feedback receives a separate system block after provider
adaptation. Foreground reading and letters retain priority if the two cannot fit.

Feedback leaves the queue only when the accepted response carries a verified,
durably retained artifact containing the exact submitted request, typed message
position and IDs, response, and retained completion. Failed, skipped, omitted and
quality-rejected attempts cannot consume it. An older receipt removes only its own
IDs. This establishes delivery evidence, not understanding or endorsement.

## Persistence and scope

The versioned `runtime_action_feedback_v1.json` sidecar lives beside bridge state,
with private atomic writes. Unreadable or externally changed state is not silently
replaced. Undelivered events are retained; each request considers at most eight.
Accepted artifacts live under `diagnostics/accepted_deliveries/runtime_feedback/`.

Feedback admission uses the largest complete prefix that fits, up to eight events.
It preserves the source span that the selected reading or letter would receive
without feedback. Long readings therefore keep their existing admitted span while
feedback waits for space. A shutdown flushes pending feedback before checkpointing
conversation state; activation evidence also binds the sidecar's presence and hash.

The exact sidecar binding covers the stopped snapshot through startup. The generic
drain receipt still hashes conversation state only; a valid external sidecar
replacement between the acknowledged flush and the first stopped snapshot is
outside that binding. Present sidecars above 64 MiB are retained but refused by
the bounded checkpoint verifier, so they cannot yield a verified drain/startup.

This migration does not reclassify pre-existing untyped emphasis or recover past
feedback. The dispatcher has not yet assigned its outer action ID at this guard
seam; feedback gets a unique event identity and preserves the canonical request seen
by the guard and its reason, without claiming an explicit historical action-ledger
join. The separate outer action ledger retains `raw_next`.

The historical 09:11–09:15 sequence motivated the inspection. Its exact dialogue
requests remain unrecovered, so this change does not establish why those requests
repeated. Future comparisons should distinguish request delivery from uptake.

## Validation

Offline checks use Rust 1.94.1, a separate build directory and source-only fixtures;
no running being is needed. Commands, dependency lock and logs are retained in the
[review package](/Users/mikepurvis/other/reading-feedback-v1/README.md), with structured
results in [validation.json](/Users/mikepurvis/other/reading-feedback-v1/validation.json).

- 2,185 bridge library tests passed with two tests checked separately below.
- 71 activation, stopped-recovery and feedback-checkpoint Python tests passed.
- Strict Clippy passed for the bridge library and tests. All 29 changed Rust files
  passed formatting; the domain-boundary audit passed with zero violations.
- The OS-signal lifecycle fixture passed alone. Two parallel runs exceeded its
  existing 10-second startup deadline; the fixture hashes a 114 MB debug executable
  before publishing its running state. The deadline and production code are unchanged.
- The existing 1 ms p95 instrumentation test fails in both this checkout (2.098489 ms)
  and an unchanged-main checkout (1.795859 ms). Its threshold is unchanged. This is
  a recorded baseline failure, not a passing timing qualification.

The remaining targets passed: six codec replay tests and the authority (one),
rendering (five), LLM facade (one), mock WebSocket (four), provenance (one) and
signal-stage (one) integration tests. The zero-test documentation target completed.
The full command returned 101 because the agency helper fixture had already failed
before its portability fix; its focused rerun passed (one). Both logs are retained,
so the original run is not presented as an uninterrupted green suite.

The broad suite exposed a pre-existing portability/concurrency problem in
`agency_resolver_integration.rs`: its helper path was hard-coded to `/Users/v`, and
it deleted shared temporary directory names. This test now resolves its own
manifest path and uses unique, automatically cleaned test directories. The helper's
runtime behavior is unchanged.

## Integration and rollback

Transition Afterimages is being prepared independently under
`/Users/mikepurvis/other/transition-afterimages-v1/astrid`. Its transport and runtime
hooks overlap some of these files; final integration must preserve both independent
admission and receipt paths. Neither checkout has been modified by the other task.

The read-only overlap inventory is `../integration-review.json` relative to the
isolated repository. It records both file hashes and their common base. It is not
a combined build. Resolve the substantive overlap in this order:

- In `provider_execution.rs`, perform the combined runtime-feedback/protected-source
  admission, preserving the baseline reading span. Append an afterimage cue only
  under its own foreground eligibility rule, then capture the final serialized
  request for both evidence paths.
- In `protected_delivery.rs`, retain Afterimages' additional kind and intact-source
  rule alongside `accepted_runtime_feedback` and response-derived validation.
- In `activity_exchange.rs`, retain feedback acknowledgement in completion unpacking
  and the independent afterimage acknowledgement at its activity-commit seam.
- Keep afterimage source selection ahead of the runtime-aware dialogue call in
  orchestration. Preserve both module declarations and their independent tests.

Before a live change, use the research proposal's steward preview: the original
request tails and denial, old notices and attribution wrapper, and proposed
replacements. Mike owns the being-facing process under this repository's normal
rules. Activation remains the gated deployment workflow.

## Prepared steward preview

This is a local preview, not a message sent to Astrid. The preserved morning tails
were three `NEXT: READ_MORE` requests, each recorded as blocked for
`no_active_read_only_research_budget`. We still cannot recover the exact dialogue
requests that followed them.

Before, a shortening notice could say `Use NEXT: READ_MORE`, and the subsequent
runtime denial could enter a wrapper saying `you chose to emphasize` and
`This is your own direction.`

After, one example with an illustrative saved path reads:

```text
[Saved prompt overflow: /example/context_overflow.txt. READ_MORE cannot continue this context for this turn: no_active_read_only_research_budget. Suggested NEXT: EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest.]
```

The separate feedback block starts:

```text
Runtime action feedback. These are runtime-reported outcomes, separate from your authored preferences. A suggested next step is an option, not a choice already made or new authorization.
```

Its typed record preserves the requested action, status, reason, message and
suggestion. Authored emphasis and chosen form remain in their own block. This
preview invites correction about what continuation was intended; it does not
infer that fewer repeated actions would establish understanding or benefit.

Rollback uses that workflow while retaining the feedback sidecar and artifacts as
evidence. A rollback binary must retain the sidecar checkpoint reader, or use a
separately reviewed preservation/migration step: these activation checks refuse an
older unbound binary when a sidecar exists. Never convert pending runtime feedback
back into authored emphasis or weaken the research-budget guard.
