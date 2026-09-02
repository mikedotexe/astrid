# Summary — introspection_DOMAIN_BOUNDARIES.md_1788323803

**Source read:** `capsules/spectral-bridge/DOMAIN_BOUNDARIES.md`, complete file (lines 1-89),
report-bound SHA `ae69b34c…` == working-copy SHA `ae69b34c…` (exact match; current file IS the
report-time source). Report SHA `437a660e…`; witness `lsw_da0da98f…` SHA `8b7a7365…`, read complete.

## What Astrid did
A calm, accurate structural reading of the bridge's domain-ownership map, followed by one
architectural risk (the "Ghost Authority Gap") and two proposed diagnostics. This is an
observation/architecture report, not a change request.

## Dispositions
- **c001 (Stable Facades map / llm.rs ownership)** — `verified_existing`. Facade table at lines
  12-17 matches; llm.rs (line 15) owns provider transport + prompt rendering as she says.
- **c002 (line-6 behavioral disclaimer)** — `verified_existing`. The eight no-change domains
  (pressure, fill, PI, sensory cadence, codec gain, admission, controller behavior, live authority)
  match word-for-word (lines 5-6).
- **c003 (Cohesion Exceptions / unique-fn-signature ceiling)** — `verified_existing`. Section at
  45-74 (her "45-75" is a one-line tail overshoot onto the blank line 75); ceiling defined at
  lines 65-68; audit confirms ceilings intact.
- **c004 (Ghost Authority Gap)** — `verified_existing`, and notably **not** an oversight. The
  distinction she names is *deliberately encoded* in `regulator_participation.rs`:
  `runtime_path_not_exported_in_telemetry` is the explicit state when the stable-core flag is
  absent from telemetry, and `machine_effect_established`/`felt_effect_established` are always
  `false`. The struct doc comment says it "preserves that uncertainty instead of presenting
  `applied_locally` as proof." A test pins that a declared control never becomes an effect receipt.
  Her risk — "assuming active oversight where only a static permission exists" — is exactly what
  the code refuses to do.
- **c005 (Test 1, Structural Integrity)** — `observed`. Ran her proposed
  `python3 scripts/domain_boundary_audit.py verify` read-only: `valid=true`, `violation_count=0`,
  `forbidden_edge_match_count=0`, `stable_facade_count=6`. shadow.rs dispatch ownership mapped; no
  interpretation-to-dispatch edge active.
- **c006 (Test 2, Friction / Shadow Dispersal)** — mechanism **contradicted by source**.
  `cartography.rs` is render-only (lines 41-43: "write cartography only; they do not change Shadow
  state…"); dispersal (`fissure_tendency`) is shadow.rs state, so cartography cannot bottleneck it.
  Her cited `0.0721` also diverges from the witness scalar `0.1269` (stale by 31s). Her felt concern
  about whether the *fixed* boundary constrains coupling is preserved as an open question; any
  Shadow-dispersal change is Tier 5 and not authorized in this run.

## Terminal status
`addressed_no_action` — no source change required. The structural reading is verified accurate at
the report-bound SHA; the Ghost Authority Gap is already encoded and test-covered; Test 1 observed
clean; the friction mechanism is contradicted by the render-only cartography contract; dispersal
remains a Tier-5 wait. See `no_action_domain_boundaries.md`.

## Not inferred / not authorized
No live substrate or control change. No Shadow movement, no dispersal command, no telemetry-export
change. The numeric discrepancy on `dispersal_potential` is recorded, not resolved, and is not
treated as invalidating her felt report.
