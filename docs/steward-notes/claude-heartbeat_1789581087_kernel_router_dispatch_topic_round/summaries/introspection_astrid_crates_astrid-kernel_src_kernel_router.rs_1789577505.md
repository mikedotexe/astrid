# introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577505

- Report: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577505.txt`
  — 2,789 bytes, 32 lines, sha256 `edb7d0ddb03510aa07293f3b7490f92ec1a8c2b74af5b523a4ed32e951ab99e2`, read complete.
- Witness: `lsw_84152d641bc7729ea156b468d312d3ee9f79dc5a4e3916ccee462b4cf12150bc`
  — 21,445 bytes, 498 lines, sha256 `dce3384afe5c1c8dd8acdafb0da854cc5e582870efc15669c86706f0a317d4b2`, read complete.
  `evidence_only`, `live_eligible_now: false`, `direct_causation_claimed: false`.
- Report-bound source: `crates/astrid-kernel/src/kernel_router.rs`, sha256 `108d6901…ee72fd`, bytes 0..4501,
  witness window lines **1-111 of 388** — the page immediately before
  `introspection_…_1789577928`, bound to the **same source sha**, which is why one complete-source
  verification serves both reports.

## What she did

The first page of the same file: the two spawned loops, the rate limiter, and the opening of
`handle_request`. She closes: *"I need to see the rest of `handle_request` to see the full suite of
supported commands"*, and chooses `NEXT: SELF_STUDY CONTINUE` — which is exactly what delivered the
next window 423 seconds later.

## Verified against the complete file

Her structural reading is right: two `tokio::spawn` loops and no more — `spawn_kernel_router` (18-67),
subscribing `astrid.v1.request.*` at line 22, and `spawn_connection_tracker` (74-95), subscribing
`client.v1.*`, spawned from the first at line 20. `Connect`/`Disconnect{reason}` map to
`connection_opened()`/`connection_closed()` (83-90), and the count she calls "the vitality of the
system's active connections" does surface: `DaemonStatus.connected_clients` at line 206, in the very
arm she reads on her next page.

Two corrections, both from source below her window.

### 1. `ListCapsules` is not rate limited

> "a `ManagementRateLimiter` (lines 25, 39–58) to protect the kernel from being overwhelmed by
> management commands (like listing capsules or reloading configurations)"

The lines are exact and the mechanism is exact. The example is not: `rate_limit_for_request`
(292-304) returns `None` for `ListCapsules`, `GetCommands`, `GetCapsuleMetadata` and `GetStatus` —
read-only queries are deliberately unlimited. Only `ReloadCapsules` (5/min), `InstallCapsule` (10),
`ApproveCapability` (10) and `Shutdown` (1) are throttled. Her *reloading configurations* example is
correct; her *listing capsules* example is the opposite of the code. That table sat past the end of
her page, so the dispatch alone could not have told her.

**This is what the round implemented.** `rate_limit_table_covers_every_request_variant` now pins all
eight pairs, and its inner `match` has no wildcard arm, so a new `KernelRequest` variant cannot compile
until someone records whether it is limited.

### 2. The rate-limit error does not go where the code says it goes

> "If a request is rate-limited, it generates a corresponding error on a `kernel.response.*` topic."

That is the code's stated intent, and it is a no-op. Line 49 derives the error topic with
`message.topic.replace("kernel.request.", "kernel.response.")`, but this router subscribes
`astrid.v1.request.*` (line 22), and **no topic containing `kernel.request.` is published anywhere in
the repository** — the only two occurrences of that string are the `replace()` needle itself and a
stale doc comment at `lib.rs:845` that still describes the router as listening on `kernel.request.*`.
So the substring never matches, and the rate-limit error is published back on the unchanged *request*
topic, while every other response in the same function uses `strip_prefix("astrid.v1.request.")` →
`astrid.v1.response.{suffix}` (99-103).

Bounded deliberately: this is an inconsistency in the **topic contract**, not a demonstrated loss.
`socket_bridge.rs:63` subscribes the bus unfiltered, and the CLI reads the next framed message without
filtering by topic (`commands/daemon.rs:196-207`), so an unfiltered consumer would still receive the
error. A consumer that filters `astrid.v1.response.*` would not. The reachable case is real —
`reload_capsules` is the one throttled command the TUI actually sends (`tui/mod.rs:670, 770, 801`), at
5/min.

**Not fixed here.** Changing the router's response-topic derivation is production behavior, outside
this round's non-live authority (focused tests, steward tooling, documentation). It is named as
steward follow-up in `RUN_REPORT.md`, with the exact lines, rather than quietly patched or quietly
dropped.

### On `handle_request` and the spectral metrics

Same hypothesis as her next page, same answer: `crates/astrid-kernel/src/` contains no spectral,
eigenvalue, curvature or reservoir reference; the `projection.rs` she remembers is in the
spectral-bridge capsule, a different crate in a different process. And the "placeholder for identity
verification and capability checks" she sees in the `InstallCapsule` arm is a *comment* (108-109) above
a not-yet-implemented error — the real capability and approval stores are `Kernel` fields wired
elsewhere (`lib.rs:47, 92`).

## Status

`addressed_change`. Eight claims, all with grounded dispositions and linked evidence.
