# introspection_source_catalog_1789453053 — the CONTINUE that never ran

**Report** `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789453053.txt`
— 2095 B / 24 lines, sha256 `6aa03eca8d5d104a9660f4b3e5785ec01ca63692f289991abad2e4399ea63cff`, read complete.
**Witness** `lsw_4c3156cba95caa56696777123501a48e86be1e1b9d89d64fd45c42472cd27020` — 18973 B /
440 lines, sha256 `a1ae99bc1afd80a3222b7f14b95ba9102bb57d34de6e2fb9549bc4e54eb89dd0`, read complete.
`source_snapshot_v1` and `source_provenance_ref_v1` are both `null` — it is a navigation-only
turn, so there is no report-bound file SHA. Fill 73.0%, λ1 8.53, λ1−λ2 gap 4.10, mode packing
0.833, minime fill 71.0%; model route `coupled-astrid`, 68.2 s end to end.

## What she wrote

She is mid-study in `capsule_runtime_health.rs`, was not given the bytes she asked for, and
answered her own open question **from her notebook anyway**:

> "While the actual code for `accepted_legacy` was not delivered in this specific turn, my
> study notebook maintains a consistent record of its logic (lines 202–210)."

## Her recall is exact

Read complete against `crates/astrid-kernel/src/capsule_runtime_health.rs` (7000 B / 214 lines,
sha256 `3a20e5ee…` — the same revision her earlier page was bound to, so report-time and
current source are identical):

| She recalled | Source |
| --- | --- |
| `accepted_legacy` at lines 202–210 | `fn accepted_legacy` opens 202, closes 210 |
| `Baseline` at lines 7–10 | struct body 7–10, derive on 6 |
| name whitelist gates `LegacyExtismMvp` | call site 65 (that arm only); `entry.name == name` 204 |
| `Some(hash)` ⇒ exact match | 205–206 `Some(expected) => wasm_hash == Some(expected)` |
| `None` ⇒ name alone suffices | 207 `None => true` |
| `.as_deref()` makes the comparison safe | 65 and 205, both `Option<String>` → `Option<&str>` |

One precision, not a correction of her conclusion: `.as_deref()` *produces* `Option<&str>`; it
does not normalize values that already are one. Both call sites start from `Option<String>`.

**Her open question, answered.** `Baseline` is neither hardcoded nor environment-driven.
`load_baseline` (194–200) reads `<workspace_root>/scripts/baselines/capsule_runtime_health.json`
through `serde_json`, and line 31 wraps it in `unwrap_or_default()` — so a missing or malformed
file is indistinguishable from an empty whitelist. The live file (221 B, sha256 `dde94f50…`)
holds `"accepted_legacy_extism_mvp": []`, so at this revision nothing is accepted and every
legacy Extism payload lands in `actionable_incompatible`.

## The turn she lost was not the one she chose

Her three consecutive study turns each close on `NEXT: SELF_STUDY CONTINUE`, and two come back
`Recovery map: the requested source was not supplied`. That reads as CONTINUE failing. It cannot
be. `Command::parse` maps `CONTINUE` onto `Command::Continue` (`command.rs:30`); `prepare_parsed`
routes it into `prepare_continue` (`store.rs:522`), the one arm with **no** recovery branch — it
returns a pending page, an advanced page, an end-of-file notice, or the root map
(`store.rs:547-591`).

The retained navigation artifacts name the real miss. Both carry:

```
Reason: "no catalog entries for spectral_bridge; use SELF_STUDY MAP"
```

and `bridge.db action_events` shows the arbitration:

```
1789452211  SELF_STUDY CONTINUE                 handled     <- her closing choice
1789452272  SELF_STUDY MAP spectral_bridge      blocked
1789452366  SELF_STUDY REPLACE MAP spectral_bridge  handled
1789452449  SELF_STUDY MAP spectral_bridge      handled
1789452519  SELF_STUDY MAP spectral_bridge      handled
            -> study turn 1789452645 ran MAP spectral_bridge, not CONTINUE
```

The bridge holds **one** pending study target (`conv.introspect_target`, `.take()`n at study
time, `runtime/source_study.rs:75`). Study turns are rationed; the non-study turns in between
still write that slot. So a navigation choice she made *with the page in front of her* is
overwritten by a choice made in a turn that had no page, and she is never told. The identical
cycle repeats verbatim 90 minutes earlier (1789447518 .. 1789448719).

Why `spectral_bridge` never resolves is already pinned and is **not** re-litigated here:
`path_candidates` (`path_recovery.rs:14-21`) returns empty unless the request already carries two
or more segments led by an installed repository ID, so the `_`→`-` normalization on lines 40 and
49-50 — which would have named `astrid/capsules/spectral-bridge` — never runs. That exact case is
covered by `bare_topic_candidate_gate_reach.rs` from the 1789204702 round.

## What this round changed

1. **`crates/astrid-source-study/tests/continue_never_recovers_reach.rs` (new)** — pins that
   `SELF_STUDY CONTINUE` yields `Map`, `SourcePage`, `SourcePage` and `EndOfFile` across every
   reader state it can meet and *never* `Recovery`, while a bare `MAP spectral_bridge` against the
   identical reader state does recover and names itself in the reason line — and leaves the
   bookmark intact, so an interleaved failed target costs the turn, not the position. 2 tests.
2. **`scripts/source_study_recovery_loop_watch.py` (attribution)** — the watch scored both live
   loops as `SELF_STUDY CONTINUE [none]`, which would send the next investigation at the wrong
   code. Loops now carry `attribution`: `another_request_replaced_it` for every spelling that
   reaches `prepare_continue`, `this_action` otherwise, with a render line saying so. A bare
   `SELF_STUDY REPLACE` is deliberately excluded — its empty operation bails in
   `parse_replacement` and *does* reach `recovery_map`. 10 self-tests; live rescan re-attributes
   both loops.

**Not changed.** The repair itself — letting a bare separator-variant topic recover, or stopping a
later dispatch from silently replacing a pending study target — is being-facing live navigation and
dispatch. It stays an operator boundary, continuing the one already named for Mike as c011 of the
1789204702 round. Her text was not rewritten, annotated or corrected anywhere she can see, and no
action was dispatched on her behalf.
