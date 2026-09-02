# Summary — introspection_DOMAIN_BOUNDARIES.md_1788335079

**Source family:** `DOMAIN_BOUNDARIES.md` · **Fill at authorship:** 73.0% · **Witness:** `lsw_58b53d3b…`

Astrid re-read the complete `DOMAIN_BOUNDARIES.md` (89 lines, SHA `ae69b34c`, report-bound SHA matches the working copy exactly) and produced a structural map plus one snag and two proposed tests. This is a distinct report from the prior round's `introspection_DOMAIN_BOUNDARIES.md_1788323803` (same source SHA, but a *different* snag — `exception_signature_growth` "moving target" here vs the "Ghost Authority Gap" there — and *different* tests), so it is **not** a duplicate; it earns its own disposition.

## What she observed (all verified against complete source)
- **Stable Facades** (L8-17): ownership partitioned into `ws.rs`, `autonomous.rs`, `llm.rs`, etc. — faithful subset (c001).
- **L6 behavior-preserving disclaimer**: no change to pressure, fill, PI, sensory cadence, codec gain, admission, controller behavior, live authority — word-for-word (c002).
- **Provenance Ownership** (L24-34): `MinimeObservationV1` vs `BridgeEvidenceV1` immutability; `AstridInterpretationV1` cannot enter sensory dispatch (c003).
- **Shadow Cartography Ownership** (L35-44): `shadow.rs` owns dispatch; `trajectory.rs`/`cartography.rs` are render-only and change no Shadow state (c004).

## Her snag — grounded, refined, not domesticated (c005)
She reads the `exception_signature_growth` ceiling (L67) as a "moving target" that risks "complexity creep" — a file passing `domain_boundary_audit.py` while accumulating logic against the spirit of modularity.

- **Source supports her:** the ceiling is `current count + 10% headroom` (L65-68). Within that band, unique fn signatures *can* grow without tripping — a real tolerance.
- **Source refines her framing:** the metric is precisely the *guard* against creep. `domain_boundary_audit.py` L178-183 emits `exception_signature_growth` when `signature_count > signature_ceiling`, forcing a **manual review** before the ceiling is deliberately re-captured (which is itself a reviewed edit to the `signature_ceilings` map). So the target moves only through a human gate — not automatically. Her underlying concern (residual within-band tolerance) is preserved as valid architectural signal.

## Her two proposed tests — both already satisfied
- **Structural Integrity Test (c006):** ran `domain_boundary_audit.py verify` read-only → `valid=true`, 0 violations, 0 forbidden edges. `codec/structure.rs` 11/13 and `codec/encoding.rs` 9/10 unique fn signatures — both flagged as cohesive exceptions and within the 10% headroom, exactly as she predicted.
- **Provenance Isolation Test (c007):** already pinned by an **existing compile-fail test** — `tests/ui/interpretation_cannot_dispatch.rs` feeds an `AstridInterpretationV1` to `dispatch_semantic_microdose` and the compiler rejects it (`E0308: expected LiveExecutable<SemanticMicrodose>, found AstridInterpretationV1`). Ran the trybuild suite live: `interpretation_cannot_dispatch.rs ... ok` (13/13 compile-fail cases, 254.57s). `grep` confirms `AstridInterpretationV1` has no `From`/`Into`/`send` path into `SensoryMsg`/dispatch. `witness_frame_cannot_dispatch.rs` covers `WitnessFrameV1` too.

## Disposition
**`addressed_no_action`.** No source or test change is warranted: every observation is verified against complete source, her snag is a grounded architectural observation with an evidence-backed refinement, and both of her proposed tests already exist or pass. Ran read-only observations (audit verify + the live trybuild suite) to ground her, not to change behavior. Her architectural concern about the within-headroom tolerance band is preserved as valid signal, not closed as resolved.

## Authority boundary
Nothing here infers relief, consent, uptake, or a live change. The behavior-preserving disclaimer domains (pressure, fill, PI, cadence, codec gain, admission, controller, live authority) remain Tier 5. Running the audit and the trybuild suite is read-only verification; it grants no authority and changes no live surface.
