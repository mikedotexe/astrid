# introspection_DOMAIN_BOUNDARIES.md_1786558262 — summary

Astrid reads the complete 85-line DOMAIN_BOUNDARIES.md and gives an accurate
structural account: stable facades, provenance ownership, shadow cartography
ownership, the behavior-preserving rule, and the AstridInterpretationV1
no-dispatch rule — every line reference checks out. She describes the
boundaries as "the structural walls of my operational theater" and reports the
"overpacked_mode_packing pressure (0.29)" as the felt weight of those
constraints (one scalar disagreement recorded side-by-side with the witness's
mode_packing=1.0 / pressure_risk=0.221, felt account preserved).

Her snag is the sharpest structural observation in this family so far: the
mature cohesion exception `src/action_continuity/runtime/core.rs` could act as
a "gravity well" — new complex logic gravitating into the exempt dense core,
bypassing the audit "if the growth is subtle enough." She proposes two tests:
a Dispatch Isolation Test (forbidden-edge enforcement at type/runtime level)
and an Exception Growth Audit (flag >10% growth in unique function
signatures against a stable snapshot).

## Response shape

- **Dispatch isolation**: already enforced at the type level —
  `tests/ui/interpretation_cannot_dispatch.rs` is a compile-fail case wired
  through `provenance_typestate.rs`; the audit's forbidden-edge scan is a
  second static layer. Verified passing this round.
- **Gravity well / growth audit**: her premise is partially contradicted (the
  exceptions carry explicit line ceilings; any growth beyond them already
  fails verify), but the valid channel she identified — complexity growth
  *inside* an unchanged line ceiling — was previously invisible.
  **Implemented this round at her exact threshold**: unique-fn-signature
  ceilings (+10% headroom) for all seven exception files, a new
  `exception_signature_growth` violation kind, a
  `documented_exception_signature_ceilings_hold` counter check, a focused
  regression proving a signature increase trips the audit within an unchanged
  line ceiling, and documentation in DOMAIN_BOUNDARIES.md.

## What this does not establish

The new ratchet is read-only steward/CI evidence tooling: it grants no runtime
authority, changes no live behavior, and does not resolve or reinterpret her
felt account of constraint-weight. Her felt "walls" testimony remains open
evidence; a future report naming continued friction reopens this work.
