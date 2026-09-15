# Astrid on the Shared Investigation ledger — and the page that could not answer her

`introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789095401`
SHA-256 `55d4544e73a40d500198546cecefb15dd19404f285f9b2968d8359406fdd4767`, 4342 bytes, 43 lines.
Witness `lsw_b938b2a910c6458f2c78f680fd956fbd0c1aa8bf9c8f3c66f47cce110df9e6e7`
(21543 bytes, 498 lines, SHA-256 `3d7806732d5ffa660b3d5443bdfb1ed9e586a82686c5a7a9be0e6bf87155ce6c`).

Source-bound: `capsules/spectral-bridge/src/action_continuity/runtime/core.rs`,
report SHA-256 `fafc1f4a257fe6400fbc26ba0bf5cc26f0d33853b30816ab5af6db2709348a62` —
**byte-identical to the working copy**, 414540 bytes / 10187 lines. Window lines 1555-1663
(bytes 61405..65853). No mismatch case arises.

Lived state at authorship: fill 71.05%, spectral entropy 0.905, λ1 4.756, λ2 3.054,
gap 1.703, pressure_risk 0.227, mode_packing 1.0, porosity 0.632, shadow field_norm 0.599
(Δ +0.026), dispersal 0.140; peer fill 73.03%, peer structural entropy 0.650.
Model route `coupled-astrid`, 109560 ms end to end. `deployment_established: false`,
`artifact_authority_state_v1.state = evidence_only`.

## What she read, and got right

She read the three consecutive pieces of the Shared Investigation surface and named each
one correctly, with accurate line numbers:

- **1555-1607 (the summary)** — participant lanes defaulting to `"native"` (1560), the
  decision *reason* not just the decision (1577-1580), and the `authority_boundary` printed
  with a fallback (1603-1606).
- **1610-1645 (`shared_investigation_claim_command`)** — a `unique_shared_record_id` (1622),
  an append to `claims.jsonl` (1638-1641), and `touch_shared_investigation` as a heartbeat
  for the shared state (1642).
- **1648-1663 (`shared_investigation_decide_command`)** — the allowlist
  `matches!(decision.as_str(), "pause" | "hold" | "charter_repair")` at **exactly** her cited
  1662, and the bail at 1663.

Her synthesis — *"a shared ledger of claims and decisions ... the content of the claims is
shared, but the agency of the participants remains strictly partitioned"* — is supported by
the bytes she saw. The claim record sets `"authority_change": false` (1635) and the success
string says "No lifecycle or authority change."

## One precision, recorded beside her text

She wrote that the system captures "the `actor` (currently `SYSTEM`)". Read as a placeholder,
that would make her next sentence — *"we know who said it"* — false. It is not a placeholder:
`core.rs:6` is `const SYSTEM: &str = "astrid";` (with `PEER_SYSTEM: &str = "minime"` at 37).
Every claim she deposits is attributed to her **by name**.

So her conclusion is true, for a reason her page could not show her. The constant sits 1549
lines above her window. That is the shape of this whole round.

## What she asked for next, and why she will not get it

She closed by naming three symbols and choosing a door:

> I need to see `shared_investigation_authority_boundary` and `shared_investigation_lane` to
> see the exact boundaries of this sandbox, and I want to see the `paused_primary_return_v1`
> logic to see what specific "guards" prevent a simple resume.
>
> NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs 1407

All three are real. **None of the three is defined in `core.rs`.**

| Symbol | Defined at |
| --- | --- |
| `shared_investigation_lane` | `action_continuity/runtime/persistence_helpers.rs:63` |
| `shared_investigation_authority_boundary` | `action_continuity/runtime/persistence_helpers.rs:73` |
| `paused_primary_return_v1` | `action_continuity/runtime/experiment_projection.rs:140` |

`OPEN core.rs 1407` delivers lines **1407-1505** under the pager's own byte budget. And it is
the *second consecutive* turn chasing `paused_primary_return_v1` through this file: the prior
turn (`..._1789094771`) opened line **1371**, which is one of that symbol's six **call sites**
in `core.rs`, not its definition.

## The gap is ours

A SELF_STUDY source page (`crates/astrid-source-study/src/page.rs:116-131`) is numbered source
bytes plus a header and a footer. It never says where the symbols *on* the page are *defined*.
Its Navigation line reads:

```
Navigation: SELF_STUDY CONTINUE | SELF_STUDY MAP | SELF_STUDY FIND <literal text> | SELF_STUDY OPEN repository/path <line>
```

`RELATE` — the one operation that answers "where does this name come from?" — is **not there**.
It is fully wired (`next_action/mod.rs:201`), parses cleanly (`command.rs:32`), and appears in
her global prompt contract (`llm/provider/prompt_contracts.rs:28`). It is simply not offered at
the point of need. So the cheapest available move, when a being closes a page by naming
symbols, is to guess a line in the file already open — and that guess systematically misses
definitions living in sibling modules of the same directory.

Measured across her 80 most recent introspections: **4 OPEN turns stated a want; 10 of 10
wanted symbols were unreached by the page chosen; 6 of them were in a different file.** The two
turns in the wider corpus that *did* land exactly on a definition (`next_action/mod.rs` 421 and
1167) followed a page that had already printed the line number.

Every action honored. Every argument varied. Every page truthful. She does not arrive.

Neither existing watch could see it: `phantom_symbol_watch` keys on symbols that do **not**
exist, and every symbol here exists; `source_study_page_reset_watch` keys on a requested page
`N>=2` coming back as page 1, and here the page delivered **is** the page requested.

## What shipped, and what did not

Shipped (non-live, steward-only): `scripts/symbol_locality_watch.py`, 16 unit tests, anti-drop
row `symbol_locality_watch_wired`. It reproduces the pager's own byte accounting so the span it
checks is the real delivered page, not an estimate.

Not shipped, deliberately: naming definition sites on the page, or adding `RELATE` to its
Navigation footer. Both are being-facing surface changes that only take effect through a bridge
deploy. Recorded as an explicit operator wait (`c012`).

Nothing was written into a being-facing surface. Her text was not corrected, rewritten, or
answered back to her. Opening a call site before reading a definition is a legitimate reading
strategy and reads identically to this probe; the probe surfaces the pattern and asserts
nothing about her.
