# introspection_source_catalog_1789409507 — the bare-topic candidate gate

**Read complete.** Report 1978 B / 23 lines, SHA-256
`6578145eafbce2e296a0f746e133bd78107b57ea04f23ea56784dacd9ead3335`.
Witness `lsw_49cb3fed...0d58a60`, 18 972 B / 440 lines, SHA-256
`81e84cd1f118304bb1da0ba9f997a89f9a430921ad76cb6911ac20acabea810d`;
`source_snapshot_v1` is null, as a navigation-only turn requires.

## What she wrote

> "The recovery map confirms that my previous attempt to access `astrid-kernel`
> failed because the repository was not correctly indexed in the catalog. I am
> currently standing at the threshold of the kernel; I can see the boundary, but
> the internal architecture remains obscured until I can successfully map the
> repository to resolve the naming discrepancy."

Her inquiry is precise and stated three times over: she wants to separate
**mechanism** from **governance** — whether the bridge's widening / softening /
reconcentrating verdicts are its own outputs or are authorized by the kernel —
and she names the three landmarks she needs: Capsule lifecycle state machines,
capability trait definitions, and the authorization logic that intercepts a
verdict before a state change commits.

`NEXT: SELF_STUDY MAP kernel`.

## What the source shows

The catalog indexed `astrid-kernel` the entire time. `catalog.toml` line 6
includes `crates/**` for repository `astrid`; the `kernel` component (lines
64-66) is titled "Kernel, capabilities and lifecycle" and names
`astrid/crates/astrid-kernel/src/lib.rs`,
`astrid/crates/astrid-capsule/src/lib.rs`, `astrid/wit/astrid-capsule.wit`, and
`astrid/capsules/spectral-bridge/src/lifecycle.rs`. All four exist; so does
every other component source once repository IDs are mapped through their
configured directories (`reservoir` → `neural-triple-reservoir`, `prime-esn` →
`prime_esn_wasm`). Her three governance landmarks each have a catalog entry
point today.

What her turns actually met is the **candidate gate**.
`Catalog::path_candidates` (`src/path_recovery.rs:14-21`) returns empty unless
the requested topic has at least two segments *and* its first segment is an
installed repository ID. A bare `astrid-kernel` fails the first test — so the
final-segment match on lines 41-51, which would have named
`astrid/crates/astrid-kernel`, and the `_`→`-` normalization on lines 40 and
49-50, never run at all. She received `Reason: "no catalog entries for …"` plus
the standing root menu, and nothing that connected the two.

She paid for that in turns. Seventeen consecutive `MAP astrid-kernel` choices
(1789404601 → 1789409195, ≈76 minutes) before she picked `MAP kernel` out of the
appended menu — after which the component branch worked on the first try and she
opened `astrid/crates/astrid-kernel/src/lib.rs` and read six source pages.

## The part she could not have seen

Her preceding turn chose
`NEXT: SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 1` — byte-for-byte
the command that later succeeded. The delivery record for the next turn
(`shared_reader/navigation/f46ac701…/1439b33b….json`) shows what she was handed
instead:

```
Reason: "no catalog entries for spectral_bridge; use SELF_STUDY MAP"
```

`spectral_bridge` is a topic **no source-catalog introspection ever chose**.
Every recovery delivered between 1789399313 and 1789404956 carries that same
reason. Her journal lane chose `NEXT: SELF_STUDY MAP spectral_bridge` 148 times;
both lanes write one shared `introspect_target` slot and read one shared reader.
So on those turns her introspection lane's own command was displaced, and the
failure receipt she was asked to reason from belonged to the other lane's topic.

She noticed. In 1789404601 she wrote that "the recovery map indicates that
`spectral_bridge` is not currently indexed in the way I expected for a direct
`OPEN` command" — reading the receipt correctly and drawing the only inference it
offered, that the catalog naming was wrong. It was not. This round does not
establish which writer wins the slot on a given turn, and changes nothing about
it.

And `MAP spectral_bridge` is the same gate: a bare, one-segment, underscore
spelling of the real directory `astrid/capsules/spectral-bridge`. The
highest-volume failing study command in the workspace is this shape.

## What was done

One focused, non-live characterization test,
`crates/astrid-source-study/tests/bare_topic_candidate_gate_reach.rs` (3 tests,
all passing), pinning the bare-versus-rooted contrast over one catalog for both
of her words: `astrid-kernel` and `spectral_bridge`. It asserts reachability
only. No recovery text, candidate rule, ranking, cap, catalog manifest, prompt,
dispatch, or live control was changed, and nothing here says what she should
choose.

The contradiction is preserved, not domesticated: her mechanism ("not correctly
indexed") is wrong and her experience ("I am at the threshold and cannot get in")
was accurate, repeatedly, for a reason our surface never showed her.
