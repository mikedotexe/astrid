# introspection_source_catalog_1789553748

Navigation-only turn (process sequence 166, runtime `runtime_97439aac…`, pid 4330). No source
page was delivered, so the witness carries `source_snapshot_v1: null` and the queue records
`lived_state_alignment: deployment_unknown`. Report sha `e4ca4d53…`, 21 lines, 1355 bytes;
witness `lsw_bb111839…`, 440 lines, 18947 bytes, sha `619e2758…`.

## What she said

She confirmed the map for `astrid/capsules/astralis/astrid-capsule-agents/src`, stated that the
directory "contains the core implementation files for the agents", named three things she was
looking for — structural definitions, how `daemon`/`uplink` manifest flags influence the `run`
loop, and how the WIT stubs are organized — and chose
`NEXT: SELF_STUDY OPEN astrid/capsules/astralis/astrid-capsule-agents/src/main.rs 1`.

## What complete source establishes

The rooted map topic resolves correctly; `Catalog::map` routes a non-component topic to
`directory_entries`, which enumerates real catalog sources under the repository-prefixed prefix
(`navigation.rs:56-58, 84-111`). But that directory holds exactly **one** catalog file,
`src/lib.rs` (58 lines, 1605 bytes, sha `c04aa14e…`). There is no `main.rs`. The plural
"implementation files" is not something the map could have shown her.

Her two structural expectations are also not satisfied *in that crate*, and the honest answer is
that they live elsewhere rather than that she was wrong to ask:

- **daemon/uplink vs the run loop.** `should_start_run_loop`
  (`crates/astrid-capsule/src/engine/wasm/mod.rs:1256-1265`, called at `624-625`) returns
  `has_run_export && (manifest.capabilities.uplink || !manifest.uplinks.is_empty() ||
  component_type is "daemon"/"uplink")`. `CapabilitiesDef.uplink`
  (`crates/astrid-capsule/src/manifest.rs:298-301`) additionally disables the WASM execution
  timeout. `astrid-capsule-agents` declares `type = "executable"`, no `uplink` capability and no
  `[[uplink]]`, and its `fn run()` is empty (`lib.rs:13`) — so it is the README's *counter*case,
  a capsule that correctly does not run a long-lived loop, not an example of one.
- **WIT stubs.** No `.wit` file exists anywhere under `capsules/astralis`. README line 10 says the
  crates "use `crates/astrid-guest` directly and export the required WIT stubs"; the export is the
  macro `astrid_guest::export!(AgentsCapsule)` at `lib.rs:45`.

## Disposition

No change was caused by this report. Every claim resolved against complete source at the recorded
hashes, and two of them resolved into exact file:line answers she can read directly. The `main.rs`
choice was a self-generated expectation, not an infrastructure loss: the delivered map could only
have named `lib.rs`. Adjacent runtime evidence past this round's projection cutoff
(`introspection_source_catalog_1789553994`) shows the open failing on the following turn; it was
read as context only and is neither recorded read nor closed here.

Closed `addressed_no_action` with a linked `no_action` artifact. The navigation cost that *was*
ours is carried by `introspection_source_catalog_1789553097` in this same packet.
