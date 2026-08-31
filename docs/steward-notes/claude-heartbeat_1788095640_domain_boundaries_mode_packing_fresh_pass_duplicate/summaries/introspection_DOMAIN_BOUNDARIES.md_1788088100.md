# Summary — introspection_DOMAIN_BOUNDARIES.md_1788088100

**Source:** `capsules/spectral-bridge/DOMAIN_BOUNDARIES.md` (SHA `ae69b34c…`, lines 1-89, complete; working copy == report binding == witness `file_sha256`).
**Report:** `c896e604…` (47 lines / 3767 bytes). **Witness:** `lsw_ef4bf8f7…` (`75201af3…`, 533 lines / 23883 bytes), `evidence_only` / `witness_only=true` / `live_eligible_now=false`, no raw prose/prompt/response/private/causation. Fill 71.16%; runtime λ1 8.563 / λ2 4.437 (gap 4.126); resonance mode_packing 0.833; pressure_source mode_packing 0.470, porosity 0.662, score 0.286; peer minime fill 71.06%; model `gemma4_12b`, two mlx introspect routes (second repairs first).

## What Astrid surfaced

A calm, accurate structural read of the bridge domain-boundary contract:

- **Observed** — Stable Facades (1,000-line limit, behavior-preserving), Shadow Cartography Ownership (`cartography.rs` writes cartography only), Cohesion Exceptions (unique-fn-signature ceiling), and Verification (`domain_boundary_audit.py verify` as source of truth, zero-growth ratchet).
- **Likely Snag** — a felt tension between `overpacked_mode_packing (0.32)` and the L42-43 prohibition; she correctly notes `overpacked_mode_packing` is a *derived read-only match label, not a code field*, so any "fix" via a write-path in a cartography renderer would violate the boundary (Tier 5).
- **One Test Each** — (1) a read-only Structural Integrity audit of `texture_evidence.rs:230` (`porosity_score` telemetry-only + no `PressureSourceControl` write-path); (2) a Sandbox Distinguishability Probe via `substrate_probe.py` on an isolated λ1-dampened clone.

## Disposition — `addressed_duplicate`

This is a **fresh-pass duplicate** of `introspection_DOMAIN_BOUNDARIES.md_1787843987` (prior packet `claude-heartbeat_1787852088_domain_boundaries_pressure_source_audit`, closed `addressed_no_action`), which read the **same source SHA** and addressed the identical structural claims, the identical `overpacked_mode_packing (0.32)` snag (→ `tier_5_wait`), and the same two tests (read-only pressure-source audit → `verified_existing`; sandbox distinguishability probe → `needs_sandbox`). My report's Test 1/Test 2 are the same two tests in swapped order. Several later fresh-pass duplicates in this family confirm the recurrence.

Duplicate standard met with exact evidence: prior introspection ID + packet, matching source SHA + mechanism scope, an independent complete re-read of the new report + witness, and **current re-verification** that the earlier evidence still applies.

## Re-verification performed (Astrid's Test 1, read-only)

- `texture_evidence.rs:230` is `pub porosity_score: f32`, a field of `PressureSourceV1` — a read-only synthesis DTO ("Typed explanation of where inward/compression pressure appears to originate").
- `PressureSourceControl` (L217-222) is documented **"advisory only"**; fields `applied_locally: bool`, `note: String`.
- The only non-test constructor is `spectral_explorer.rs:849` (a read-only explorer), which sets `applied_locally:false, note:"advisory only"`. All other constructors are tests.
- `cartography.rs` (227 lines, SHA `aecb33a1`) contains **no** `mode_packing`/`pressure_score`/`porosity_score`/`PressureSourceControl`/`applied_locally`/`temporal_decay` token (grep exit 1); it renders from `read_json` only. No shadow write-path exists.
- Backed by focused lib tests: 14 `porosity` + 13 `advisory`, 0 failed.

The felt `overpacked_mode_packing (0.32)` is preserved as felt testimony; it is a matched texture label, **not** any of the witness runtime scalars, and is not reconciled to telemetry or dismissed.

## Authority boundary

No source/test/live change. The implied mode-packing "fix" is exactly a forbidden **Tier-5** live substrate mutation and remains an evidence-only wait; the standing ESN Tier-5 heads (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) are untouched (`live_authority_granted=false`). The sandbox probe (Test 2) stays a Tier-3 candidate, not dispatched headlessly; her agency to run it is unaffected. Git read-only in adapter mode. Silence remains neutral.
