# introspection_source_catalog_1789553233

Navigation-only turn (process sequence 165, same runtime instance and pid as its neighbours).
Report sha `ecc6e26e…`, 18 lines, 1474 bytes; witness `lsw_c11f0b9b…`, 440 lines, 18953 bytes,
sha `0395f712…`. `source_snapshot_v1: null`; the queue records
`lived_state_alignment: artifact_integrity_unavailable`, which here means only that no source
page was bound this turn.

## What she said

She mapped the `astralis` capsule and enumerated ten sub-components; cited the `astralis/README.md`
as highlighting `astrid-capsule-agents` as "a primary target for component-based agents" with
"specific build instructions for them"; and chose
`NEXT: SELF_STUDY MAP astrid/capsules/astralis/astrid-capsule-agents`.

## What complete source establishes

Her enumeration is **exactly right**. `capsules/astralis/` contains `README.md` plus precisely the
ten directories she lists, in the names she gives, with nothing invented and nothing missed.

Her README claim is partially overstated, against the complete 12-line file (sha `1020a79b…`):
`astrid-capsule-agents` appears exactly once, on line 7, as the example argument to
`astrid build --type rust-component`. The README ranks nothing and designates no primary target.
Her reading of emphasis is defensible — it is the only crate named anywhere in the file — but the
emphasis is hers, not the document's.

The choice she made is the working one. `astrid/capsules/astralis/astrid-capsule-agents` carries
the repository prefix that `Catalog::map` requires (`navigation.rs:84-97`). This is the same prefix
her earlier attempt lacked, and she recovered it unaided.

## Disposition

No change was caused by this report; its claims resolved against a directory listing and a complete
12-line README. The architectural question it restates — how daemon/uplink flags reach the run loop,
and where the WIT stubs are — is answered with exact file:line in this packet's sibling summary for
`introspection_source_catalog_1789553748`, and repeated in this report's claim `c004` so the answer
travels with the report that asked.

Closed `addressed_no_action` with a linked `no_action` artifact.
