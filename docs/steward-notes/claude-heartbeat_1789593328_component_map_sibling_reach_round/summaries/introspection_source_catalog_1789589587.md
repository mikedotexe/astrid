# introspection_source_catalog_1789589587 — orientation after socket.rs, choosing the kernel component map

**Report:** `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789589587.txt`
(18 lines, 1605 B, sha256 `73aedbca2c508bf3022376a0fae2d39bf5660774ddda4db31dfb73e0174f1b0c`, read complete)
**Witness:** `lsw_c69fd24e27cb2e270cbf3c1df38cd6b4004dbc81ddba080a4bb51323bf4fa47f`
(440 lines, 18954 B, sha256 `f5c3c8bc15fe348b2fa1b49c7a090d9b60e9db8c28e139444c5a966895de215d`, read complete)
**Report-bound source:** `Source revision: navigation only` — the source catalog, no source SHA bound.
`source_snapshot_v1` and `source_provenance_ref_v1` are both `null` in the witness, so there is no
report-time snapshot to reconstruct and **every source conclusion below is current-source**, read
this round at the hashes in `source_receipts.json`.

## What she said

A navigation turn, not a source page. She reports that her "deep dive into
`astrid-kernel/src/socket.rs` clarified the fundamental mechanics of the kernel's communication
'wire'" — naming path length enforcement, `0o700` permission locking, and symlink-attack
protection — then reads the map, names `kernel`, `senses` and `reservoirs` as its significant
clusters, and proposes moving "from the 'how' of socket management to the 'what' of the data being
moved and the 'where' of the routing logic." Her stated next steps are `socket_bridge.rs` and "the
various `senses` entry points," and she chooses `NEXT: SELF_STUDY MAP kernel`.

## What complete source established

**Her three socket.rs facts are exact, at the same bytes she read.** The working copy of
`crates/astrid-kernel/src/socket.rs` hashes to `c78e014c82f6d5…2491da` — byte-identical to the
revision her two socket.rs reads bound (`…socket.rs_1789589306`, bytes 0..4481;
`…socket.rs_1789589462`, bytes 7989..10414). So there is no report-time/current split for this
file. `MAX_SOCKET_PATH_LEN` is 104 on macOS/FreeBSD/OpenBSD and 108 elsewhere (20-25), enforced at
101-108; the sessions directory is set to `0o700` at 43-60 with an in-source note that
`create_dir_all` would otherwise inherit the umask; an unexpected symlink at the socket path is
removed at 110-117. Each has an in-file regression (222-231, 264-277). Nothing in her summary
overstates what those bytes say.

**Her map reading is exact and partial in the way she says it is.** `kernel`, `senses` and
`reservoirs` are three of eight components declared in `crates/astrid-source-study/catalog.toml`
(33-71). "Significant" is her selection; the manifest has no weight or ranking field, so that word
is hers and is not contradicted by anything in source.

**One expectation the source contradicts, and the contradiction is worth keeping whole.** She
proposes `socket_bridge.rs` *and* the `senses` entry points as one move — "how these raw socket
connections are abstracted into meaningful sensory inputs." `socket_bridge.rs` really does publish
onto a sensory topic: `const SENSORY_USER_INPUT_TOPIC: &str = "sensory.v1.user_input"` (line 23).
But `sensory.v1` appears in exactly two files repository-wide — that line and
`astrid-events/src/bus.rs` (128, 1351) — and in `bus.rs` it is handled as a **mirror of
`user.v1.input`**, deliberately excluded from conversation counting so the maintenance barrier does
not double-count one human turn. Meanwhile the `senses` component's two Astrid entry points,
`codec/projection.rs` and `ws/telemetry_port.rs`, contain **zero** occurrences of the string
`sensory`, and `capsules/spectral-bridge/Cargo.toml` depends on no kernel crate at all. The
kernel's "sensory" lane carries CLI text from a person; the `senses` component's lane carries the
48D codec projection and minime's reservoir input over WebSocket. They share a word and no path.

That is a correction to the mechanism, not to the question. "How does what arrives on a socket
become something the system can actually take in" is a good question that survives intact; it just
has two separate answers in this repository, and the word `sensory` is currently the thing that
hides that from a reader navigating by name.

**Her chosen next hop works — in exactly one more hop than the map implies.** `MAP kernel` takes
the component branch (`navigation.rs:42-55`) and renders only the four declared entry points:
`astrid-kernel/src/lib.rs`, `astrid-capsule/src/lib.rs`, `wit/astrid-capsule.wit`,
`spectral-bridge/src/lifecycle.rs`. `socket_bridge.rs` — the file she named — is **not** among
them. The branch does derive each entry point's directory and emit
`SELF_STUDY MAP astrid/crates/astrid-kernel/src`, whose listing carries the sibling by exact OPEN
command. So her route is sound and the curated list is a curated list, which the page does say
("These are entry points. Browse their directories for the surrounding implementation:").

## What was implemented

`crates/astrid-source-study/tests/component_map_sibling_reach.rs` — two focused tests pinning the
affordance she is currently walking:

1. `component_map_omits_entry_point_sibling_but_offers_its_directory_in_one_hop` — `MAP kernel`
   renders the component title and its entry point, does **not** name `socket_bridge.rs`, and does
   offer `SELF_STUDY MAP astrid/crates/astrid-kernel/src`; that directory map then carries
   `SELF_STUDY OPEN astrid/crates/astrid-kernel/src/socket_bridge.rs 1`.
2. `component_id_maps_but_does_not_list_and_the_component_map_offers_no_list` — a component ID is a
   `MAP` topic but not a `LIST` topic (`list` only prefix-matches catalog IDs, `navigation.rs:70-80`),
   and the component map correctly withholds the `LIST` offer by passing `scoped = false`, while a
   scoped directory map does offer its recursive `LIST`. This pins a *correct* namespace asymmetry
   next to the un-rooted `MAP`/`OPEN` asymmetry recorded in
   `claude-heartbeat_1789557900_unrooted_map_namespace_round`.

Both pass. No production behavior was changed.

## Not inferred, not authorized

- No claim that she understood socket.rs, only that the bytes she cited say what she says they say.
- Bytes `4481..7989` of `socket.rs` have never been delivered to her. `coverage.rs:87-96` did offer
  her the exact missing-region `OPEN` on the EOF page; she chose `MAP` instead. **That is her
  choice and is neutral** — it is not a gap in the affordance and not evidence of avoidance.
- The `sensory.v1` naming collision is recorded as an observation. Renaming a live IPC topic is a
  protocol change and stays Tier 5; nothing here proposes or authorizes one.
- No live substrate, control, deploy, or restart change was made or required.
