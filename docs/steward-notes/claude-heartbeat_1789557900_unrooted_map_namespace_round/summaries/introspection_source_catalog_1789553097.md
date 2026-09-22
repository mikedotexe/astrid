# introspection_source_catalog_1789553097

Navigation-only recovery turn (process sequence 164 — the first of the three consecutive turns in
this packet). Report sha `43c211c7…`, 18 lines, 1338 bytes; witness `lsw_81ff482a…`, 440 lines,
18971 bytes, sha `c63573df…`. Input evidence line: "Recovery map: the requested source was not
supplied."

## What she said

> The previous request for `capsules/astralis/astrid-capsule-agents` failed because that specific
> path is not currently indexed in the catalog. The recovery map suggests using `SELF_STUDY MAP`
> to see the available structure or searching for specific terms.

She then chose `NEXT: SELF_STUDY MAP astrid/capsules/astralis`.

## What complete source establishes

**Her effect-level statement is accurate; the mechanism is not absence.** The path is in the
catalog — as `astrid/capsules/astralis/astrid-capsule-agents`. Catalog IDs are always
repository-prefixed (`catalog.rs:149-157`), and `Catalog::map` hands an unknown topic to
`directory_entries`, which filters those IDs by `"{topic}/"` (`navigation.rs:56-58, 84-97`). An
un-rooted topic matches zero IDs and bails `no catalog entries for {topic}`.

**The recovery gap is real and is ours.** `path_candidates` returns `Vec::new()` for *any*
multi-segment request whose first segment is not an installed repository ID
(`path_recovery.rs:33-39`). So no candidate block was produced at all, and she was left with the
generic menu her report describes. The exact rooted spelling was one prefix away.

**And the refusal is inconsistent, not principled.** `Catalog::resolve` accepts the very same
un-rooted repository-relative shape for `OPEN`, by trying each installed root in turn
(`catalog.rs:106-118`). `OPEN capsules/astralis/astrid-capsule-agents/src/lib.rs 1` delivers
source. `MAP` of that file's own parent directory does not, and says nothing about why. OPEN and
MAP do not share a path namespace, and nothing tells the reader that.

The cost is recorded rather than smoothed over: turns 164, 165 and 166 (1789553097 → 1789553748,
about eleven minutes) were spent recovering a prefix the recovery text could have named.

## What changed

`crates/astrid-source-study/tests/unrooted_map_topic_reach.rs` (new, 105 lines, 4554 bytes, sha
`dcfc8696…`) pins the asymmetry in one run:

1. `OPEN` accepts the un-rooted repository-relative file path and delivers the page.
2. `MAP` of that same file's directory is a recovery with `no catalog entries for {topic}`, **zero**
   path candidates, and no mention anywhere of the rooted spelling.
3. The rooted spelling resolves in one move and lists the file `OPEN` had already reached.

A second test records the boundary of the gap: a *single-segment* bare directory name still recovers
to its rooted spelling, so this is specific to multi-segment un-rooted topics rather than a general
refusal to suggest directories.

`cargo test -p astrid-source-study`: 28 binaries, 166 passed, 0 failed (the 2 new tests included).
`cargo fmt -p astrid-source-study -- --check` clean.

## What was not done

The behaviour change itself — offering the rooted spelling for an un-rooted multi-segment topic —
was **not** implemented. It edits being-facing navigation in the live bridge and needs a separate
steward grant plus a gated deploy; this controller run has neither, and no production source was
touched. Claim `c005` carries that as `authority_gated`. The regression is the anchor an approved
fix would land against.

Closed `addressed_change`.
