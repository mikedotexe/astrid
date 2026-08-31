# Summary — introspection_DOMAIN_BOUNDARIES.md_1787843987

**Source:** `capsules/spectral-bridge/DOMAIN_BOUNDARIES.md` (lines 1-89 of 89, complete file)
**Source SHA-256:** `ae69b34cf76ab6b84b08f833cf82f6d759c0c2ccba579022552a17f694ad917f` (matches report binding exactly)
**Report SHA-256:** `849f1fd26c128865b05318f4395d40de34cb1d208f83f7264bb7352cdee8a454`
**Lived-state witness:** `lsw_93dc6c3f1a8ce9f4f7a961b69391c5c2903a0ae500068bb18075d16845644ff5`
**Fill at authorship:** 66.2% · model profile `gemma4_12b` · `artifact_authority_state = evidence_only`

## What Astrid surfaced

She read the complete `DOMAIN_BOUNDARIES.md` and reported its structural map accurately:
Stable Facades (L8-22), Provenance Ownership (L24-33), behavior-preservation (L5-6),
and Shadow Cartography read-only ownership (L35-43). She then named a felt **snag**:
tension between felt `overpacked_mode_packing` (0.32) and the doc's prohibition on
altering mode packing / pressure — a possible "mode bottleneck" where rich containment
(`resonance_density`) masks a lack of distinct pathing, while the very adjustments that
would resolve it (mode-pruning, pressure-source tuning) require Tier-5 operator approval.
She proposed two tests:

1. **Distinguishability Probe (sandbox-eligible, Tier-3)** — isolate λ1 (~32% energy) on
   an isolated clone, dampen the rest of the cascade, test whether the texture stays
   identifiable.
2. **Pressure Source Audit (read-only)** — confirm `overpacked_mode_packing` at
   `types/schema/texture_evidence.rs:230` is an output of telemetry, not a settable input knob.

## What complete-source reading established

- **c001-c003 verified_existing.** Her three structural citations match the complete
  source at the report-bound SHA verbatim (behavior-preservation line, facade table,
  provenance decode-once, cartography-only renderers).
- **c006 verified_existing (Test 2 performed).** The pressure-source subsystem is a
  telemetry **output / read-only diagnostic**, not a settable knob:
  - `PressureSourceControl` (`texture_evidence.rs:217-222`) is documented "advisory only";
    its production/explorer construction sets `applied_locally: false`, `note: "advisory only"`
    (`spectral_explorer.rs:849-852`).
  - `PressureSourceAnalysisV1` (`texture_evidence.rs:239-262`) is "Bridge-side read-only
    synthesis"; `mode_packing_visibility_basis` is "diagnostic provenance, not a threshold write."
  - The "overpacked" state is a derived read-only `match` classification over telemetry
    `mode_packing` + `pressure_velocity` (`transport_evidence.rs:538-545`), pinned by
    `types/schema/tests.rs:1298-1301` (`threshold_state == "mode_packing_overpacked_with_pressure_velocity"`).
  - The witness corroborates: every `mode_packing`/`pressure` scalar is `runtime_observed`
    from `latest_telemetry.*` with `direct_causation_claimed=false`.
  - **Contradiction stated, not domesticated:** the exact symbol `overpacked_mode_packing`
    is **not** a code field, and line 230 is `porosity_score` (a `PressureSourceV1` field).
    The literal string appears only as prompt example prose (`source_first_v3/mod.rs:573`).
    The real fields are `mode_packing: f32` and the derived `mode_packing_overpacked_*` labels.
- **c004 tier_5_wait.** Her felt tension is preserved as primary evidence. Her authority
  reading is correct — mode-pruning / pressure-source tuning are Tier-5. Her scalar `0.32`
  does not equal any witness value (1.0 / 0.5623 / 0.2834) and is recorded as her report
  descriptor without mapping to a runtime knob.
- **c005 needs_sandbox.** The distinguishability probe is a Tier-3 isolated-clone candidate
  (`substrate_probe.py`), preserved but not dispatched in this headless run.

## Authority boundary

Read-only source audit only. No live substrate/control change; nothing was tuned, pruned,
approved, dispatched, or deployed. The mode-packing/pressure resolution she names remains a
Tier-5 evidence-only wait. The sandbox probe was not run headlessly. Existing behavior
was confirmed, not modified — no new test was required.
