# introspection_source_catalog_1789437540 — the replacement we offered her could not run

Report: `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789437540.txt`
(1 701 B / 18 lines, SHA-256 `4f4e8abdd4d39fc50bed62e91338afe64bdbd69835476c1c4e35437041cb993f`, read complete)
Witness: `lsw_a8bdf400d11b5f72191e589169c270d075c871ea57c9dd7327e55baa92f9c6cf`
(18 959 B / 440 lines, SHA-256 `85dccfe61d59e6763b3af4167ccf0a57f56d6d75bff0ee1f23927ab3569f08f8`, read complete)

## What she wrote

Her header carries the delivery fact, not a complaint:

> Input evidence: Recovery map: the requested source was not supplied. No new source page is supplied this turn.

and her body says she is

> currently at the beginning of the `mcp.rs` file (which is currently missing from this turn's delivery)

and needs "to re-establish my position". She chose `NEXT: SELF_STUDY MAP astrid`.

## What her source reading got right

At the report-bound SHA `a1110a9b` her mechanism reading is exact. `McpHostEngine::load`
(mcp.rs:47) does take `ctx: &CapsuleContext`, and the `connect_dynamic` call (162-164) does not
receive it. `ctx` is used exactly once in `load`, at line 143, to resolve `[env]`.

## Where source contradicts her hypothesis — preserved, not domesticated

She inferred that "the `mcp_client` or the `McpHostEngine` itself must already hold a reference to
the context or the store." Neither does. `McpHostEngine`'s four fields (mcp.rs:22-27) are
`manifest`, `server_def`, `capsule_dir`, `mcp_client`. `SecureMcpClient`'s four fields
(secure.rs:43-52) are `client`, `capabilities: Arc<CapabilityStore>`, `audit`, `session_id`.

Her question still has an answer, and it is the one she was reaching for: the engine is built at
`loader.rs:53-65` inside `CapsuleLoader::create_capsule`, with `self.mcp_client.clone()` — a
loader-owned client that never came from a `CapsuleContext`. What authorises the dynamic
connection is `SecureMcpClient`'s own `CapabilityStore`, which is a *different* store from
`CapsuleContext::allowance_store` (context.rs:44, `Option<Arc<AllowanceStore>>`, default `None`).
`loader.rs` is 79 lines. It is the file her previous turn failed to open.

## The delivery failure is ours, and it compounded

Volition receipts, in order:

| Exchange | Action | Result |
| --- | --- | --- |
| ex-198919 (1789437177) | `SELF_STUDY OPEN astrid/crates/astrid-capsule/src/engine/mcp.rs 1` | **applied** — queued |
| ex-198920 (1789437299) | `SELF_STUDY MAP astrid/crates/astrid-capsule/src/engine/mcp.rs 1` | **blocked** — pending OPEN preserved; block text offers `SELF_STUDY REPLACE MAP astrid/crates/astrid-capsule/src/engine/mcp.rs 1` |
| ex-198921 (1789437395) | that exact replacement, verbatim | **applied** — supersedes her working OPEN |
| ex-198922 (1789437462) | the superseding MAP executes | `no catalog entries for …/mcp.rs 1`; Recovery; **zero candidates** |

The queued OPEN would have worked: reproduced read-only against the installation catalog on a
temp state, `OPEN astrid/crates/astrid-capsule/src/engine/mcp.rs 1` delivers page 1 (bytes
0..4490). She was steered off a choice that worked, onto one that could not.

The mechanism is a spelling asymmetry between two verbs over one argument. `page_suffix`
(command.rs:141-155) recognises only a ` --page N` suffix, so a bare trailing number stays inside
the MAP topic; `Command::Open` (command.rs:85-99) splits a bare trailing number off as the line.
Because the number lands in the final segment, `Catalog::path_candidates` (path_recovery.rs:44-51)
can never match `candidate.last() == parts.last()`, so the recovery that exists to spell a path
correctly is empty *exactly when the path was already correct*. Her recovery then listed her own
saved bookmark — `SELF_STUDY OPEN astrid/crates/astrid-capsule/src/engine/mcp.rs 1
[Read missing earlier bytes — Partial delivery; delivered bytes 846..11912 of 11912]` — the very
command she had just been talked out of.

`Command::parse` accepts that MAP, so a replacement suggestion that validates by parsing alone
will keep offering un-runnable commands.

## Disposition

Three reachability facts are pinned read-only in
`crates/astrid-source-study/tests/map_trailing_line_number_reach.rs`. Repairing the bridge's
replacement suggestion (`study_navigation.rs` `valid_replacement` / `handle_request`) changes
being-facing live navigation and is **not** done here; it is recorded as a named gap for a
separately authorized round.
