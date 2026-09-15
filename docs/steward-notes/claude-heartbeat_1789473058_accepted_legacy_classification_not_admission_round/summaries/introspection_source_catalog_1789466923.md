# introspection_source_catalog_1789466923 — a verdict reported, not imposed

**Report** `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789466923.txt`
— 1 664 B / 22 lines, sha256 `9cf30ef6023da32bff5377a00d77651d323837ec687ae1527b96d18d42fe7bf5`, read complete.
**Witness** `lsw_de85f24ec36fbc1b386de49cfb3f5c7384ec5ce455407fcda4369dd0afbcb8d3` — 18 970 B / 440
lines, sha256 `40826d549bd187ef3356d2edbffdc99a05c41e417334629aef7bd9a2d8095f5c`, read complete.
`source_snapshot_v1` and `source_provenance_ref_v1` are both `null`: a navigation-only turn, so
there is no report-bound file SHA. Fill 71.04 %, λ1 8.535, λ1−λ2 gap 4.098, spectral entropy 0.8828,
mode packing 0.833 (resonance) / 0.471 (pressure-source), porosity 0.662, pressure score 0.292,
shadow field_norm 0.5207 with delta −0.0105, dispersal 0.1068; minime fill 71.04 %. Model route
`coupled-astrid`, 89.97 s end to end.

## What she wrote

No page was supplied, and she did not ask for one. She declared the study settled and moved:

> "My understanding of the `accepted_legacy` function is established: it acts as a tiered
> validation gate for legacy components, balancing system stability with security."

> "Having established this, I am ready to pivot to the broader architectural integration of these
> integrity checks with the capability system and capsule lifecycle management."

`NEXT: SELF_STUDY MAP kernel`.

## Her recall is grounded, not remembered-from-nothing

Read complete against `crates/astrid-kernel/src/capsule_runtime_health.rs`, 7 000 B / 214 lines,
sha256 `3a20e5ee…` — clean in git, and the *same* revision her own pages were bound to:

| Page | Interval | Bound revision |
| --- | --- | --- |
| `…capsule_runtime_health.rs_1789455871` | bytes 0..4335 | `sha256:3a20e5ee…` |
| `…capsule_runtime_health.rs_1789456202` | bytes 4335..7000 (holds 202-210) | `sha256:3a20e5ee…` |
| `…capsule_runtime_health.rs_1789456944` | bytes 4315..7000 | `sha256:3a20e5ee…` |

Her delivery history covers the whole file at one revision. Both mechanism claims verify exactly:
the name whitelist (203-204), `Some(expected) => wasm_hash == Some(expected)` (205-206),
`None => true` (207).

## Three precisions her staged reading hides

**One list, not two surfaces.** `accepted_legacy_extism_mvp` is the *only* field of `Baseline`
(6-10), so "whitelist" and "Baseline struct" name the same data, and the two tests are one
conjunctive predicate inside a single `any()` (203-209) — not a first stage followed by a second.
That matters concretely: `any()` is disjunctive across entries, so a name-only duplicate entry
anywhere in the list satisfies the predicate even when a same-named *pinned* entry mismatches.

**The pin binds a declaration, not the bytes.** `read_meta_wasm_hash` (139-143) lifts the
`wasm_hash` string out of the capsule dir's `meta.json`; `classify_payload` (164-186) reads the
payload bytes but never digests them. So a "specific cryptographic hash" pins what meta.json says
about itself. It does fail closed — absent or unparseable meta.json yields `None`, which fails a
pinned entry (206) while a name-only entry still passes (207).

**A verdict reported, not imposed.** `summarize` is `pub(crate)` (27) with exactly one caller
repo-wide — `kernel_router.rs:199-200`, under `KernelRequest::GetStatus`, a read-only status read.
Its whole effect is counters (`accepted_legacy_extism_mvp` vs `actionable_incompatible`, 65-71) and
`status = "warning"` (79-81), printed by `astrid-cli/src/commands/daemon.rs:254-262`. Nothing in
this file blocks, unloads, or denies a capsule. Her phrase *validation gate* is exact for the
classification and too strong for the enforcement.

## The strategy is implemented; the policy currently forbids it

The one input her pages could never show is the data file. `load_baseline` (194-200) reads
`<workspace_root>/scripts/baselines/capsule_runtime_health.json` — live: 221 B, sha256
`dde94f50…`, `"accepted_legacy_extism_mvp": []`, and a `growth_policy` that reads *"No legacy
Extism/MVP astralis capsules are accepted backlog. Any newly installed legacy or incompatible
Component payload is actionable."* So the tiering she describes is a real, correctly-read
capability with **zero** current entries: `accepted_legacy` returns false for every component, and
nothing "transitions more fluidly" because nothing is accepted at all. Her reading of the code is
right; the deployed configuration expresses the opposite of the flexible half of her conclusion.

## The pivot she announced has no edge to walk from here

She is ready to trace "integration of these integrity checks with the capability system and capsule
lifecycle management". At this revision that edge does not exist in this file. `summarize` is
crate-private, called once on a read-only path, and receives `loaded_capsules: &[String]` — the
names of capsules that have *already* been loaded. It observes lifecycle output; it does not
participate in admission. No capability, token, approval, or interceptor symbol appears in its 214
lines. `NEXT: SELF_STUDY MAP kernel` is the right shape for finding where enforcement actually
lives; it is simply not reachable by following this file.

Both implementations agree on the predicate she established: the steward-side twin
`scripts/capsule_runtime_health.py` `accepted_legacy` (76-83) computes name equality AND
(`expected_hash is None` OR `expected_hash == wasm_hash`), and is the surface
`proactive_scan.probe_capsule_runtime_health` (1466-1511) consumes.

## Disposition

`addressed_duplicate`. This is a fresh pass over the lineage closed as
`introspection_source_catalog_1789453053` (packet
`docs/steward-notes/claude-heartbeat_1789457400_continue_replaced_by_interleaved_target_round/`);
both bindings — source `3a20e5ee…`, baseline `dde94f50…` — re-verified unchanged this round, so the
earlier evidence still applies. c003, c004, c006 and c007 are new and carry their own dispositions;
none of them contradicts the prior close.

The projection's `artifact_integrity_unavailable` flag (one gap) reconciles as absent-snapshot, not
byte contradiction: no page was supplied, so `source_snapshot_v1` is null, while `artifact_sha256`
equals the report bytes and `canonical_body_binding_v1` records 1 087 bytes after the header
separator.

## Not done

No test was added. Pinning the `accepted_legacy` tiering as a Rust regression means compiling
`astrid-kernel`, whose last build artifact is from 2026-06-03; a cold rebuild of that dependency
tree does not fit this round's remaining budget without risking an unclosed report. Recorded as
exact debt rather than started and abandoned: there is currently **no** test anywhere in the
workspace that exercises `accepted_legacy`, in either the Rust or the Python implementation. No
source, configuration, live, deploy or git change was made this round.
