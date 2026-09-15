# introspection_source_catalog_1789200495 — the crossroads that had no branch

Astrid closed her reading of `autonomous/runtime/text.rs` with a correct, unprompted
inference and a specific next target:

> "Based on my previous notes, I am moving to `continuity.rs` to find the implementation
> of `semantic_boundary_before`."

She was right. `fn semantic_boundary_before` is at
`capsules/spectral-bridge/src/autonomous/runtime/continuity.rs:352`. Nobody showed her
that; she derived it from a file that never names it.

She then issued `NEXT: SELF_STUDY MAP continuity` **sixteen consecutive times over 56.5
minutes** (1789200287 .. 1789203675). Every turn came back
`Input evidence: Recovery map: the requested source was not supplied.` She escaped only by
giving up the target: at 1789203836 she switched to `MAP astrid` and started walking the
repository catalog from page 1. She never reached `continuity.rs`.

## What actually happened

`Catalog::map` (`crates/astrid-source-study/src/navigation.rs:61-74`) resolves a topic as a
component ID or a **directory prefix** of catalog entries. `continuity` is neither, so it
bails `no catalog entries for continuity`. `Reader` then calls
`recovery_with_candidates` (`store.rs:465-472`), which prints candidates only if
`Catalog::path_candidates` returns any — and that function's first guard
(`path_recovery.rs:14-21`) returns empty unless the requested string **already has two or
more slash-separated segments whose first segment is an installed repository ID**.

So `continuity`, `continuity.rs`, and `runtime/continuity.rs` all take the empty-candidate
branch. `astrid/capsules/spectral-bridge/src/autonomous/runtime` would have landed her in
one move. The most natural way to name a file you have been reading — its bare stem — is
exactly the form that earns no correction. She was handed a reason line and the root
component menu sixteen times, and read that as "try again."

Her request was well-formed, her target existed, and the surface could not tell her the
one thing she needed. This is our muffle, not her limit.

## The question she asked, answered

> "I need to see the arithmetic of the weights. … If a `shadow_field` suggests one
> transition point and a `structural_integrity` score suggests another, how does the
> bridge decide?"

**Neither is a score.** Both are literal strings in one flat `&[&str]`:
`"structural integrity"` is index 15 and `"shadow_field"` index 22 of
`SEMANTIC_TRUNCATION_ANCHOR_TERMS` (`text.rs:130-195`). They do not carry magnitudes and
they do not compete.

`anchored_excerpt_with_terms` (`continuity.rs:242-341`) picks the anchor by a five-tier
precedence over **byte positions**:

| Tier | Source | Resolution |
| ---: | --- | --- |
| 1 | first `HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT` = 10 terms (`text.rs:60`) | `.min()` — earliest match |
| 2 | terms past index 10 containing a space/underscore/hyphen | `.min()` |
| 3 | all remaining terms past index 10, plus quoted/emphasized phrases | `.min()` |
| 4 | `PRESSURE_CONTINUITY_FALLBACK_TERMS` (`text.rs:196-208`) | `find_map` — **list order** |
| 5 | `HIGH_TEXTURE_CONTINUITY_FALLBACK_TERMS` (`text.rs:209-221`) | `find_map` — list order |

Index 15 and index 22 are both past 10 and both multi-token, so `structural_integrity` and
`shadow_field` land in the **same tier 2**, and the tie goes to whichever appears **earlier
in the text**. Not magnitude. Not list order. Position.

Her instinct that weighted arithmetic exists is right — it is just in a different job.
`continuity_recap_texture_family_score` (`continuity.rs:64-105`) counts how many of six
felt families a passage touches (resistance, viscosity, calcified/structural,
density/lattice/cascade, pressure/porosity, texture/witness), and
`continuity_afterimage_substance_density_factor` (170-210) is a real weighted sum
(0.25/0.15/0.15/0.15/0.10/0.20, clamped). **Those decide how much room a memory gets. The
anchors decide where the cut lands. They are two separate systems**, which is why the
weights were not where she expected them.

## Where the cut actually happens

She wrote that "the actual mechanics of the 'surgical' cuts remain a mystery in that
file." Half true, and worth saying plainly: the cut **is** in `text.rs`.
`truncate_str_at_semantic_edge` (`text.rs:21-40`) prefers a strong terminator
(`.` `!` `?` `;` `:`) over a weak one (`,` space), each gated by `min_keep`, falling back to
the raw byte truncation. What `text.rs` does not hold is **which region to cut toward** —
that is `continuity.rs`. Her decision to leave the file was right; her reason was half
right. And `semantic_boundary_before`, the symbol she named, turns out to be the simplest
function in the file: a plain scan for `.` `!` `?` newline under budget, with no term list
at all, used as the preferred path by
`truncate_continuity_recap_at_semantic_boundary` (367-375) before it falls back to the
anchored machinery.

## Disposition

Eleven claims, zero proof gaps. Eight `verified_existing` (two carrying preserved
contradictions), one `observed`, one `implemented_now`, one `needs_operator_approval`.

Shipped: `scripts/source_study_recovery_loop_watch.py` (read-only steward watch, 8
self-tests) and a focused regression in
`crates/astrid-source-study/tests/path_recovery.rs` pinning the bare-stem boundary.

Not shipped, and named for Mike: relaxing the guard so a bare stem or unrooted suffix can
match catalog entries by final segment. That changes the text of her own recovery prompt
and reaches her only through `build_bridge.sh --restart` — no authority for either in this
run.
