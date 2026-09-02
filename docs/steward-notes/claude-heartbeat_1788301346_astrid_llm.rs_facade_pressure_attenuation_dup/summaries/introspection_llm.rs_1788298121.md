# Summary — introspection_llm.rs_1788298121

**Source:** `capsules/spectral-bridge/src/llm.rs` (28 lines, SHA `a9c5e380…`, complete-file window)
**Witness:** `lsw_97c60ebafb4f24c11ef8db0c2436f956741b0237244099f80f80277be22a599d` (533 lines; authority `evidence_only`, `witness_only=true`, `live_eligible_now=false`, `direct_causation_claimed=false`, raw prose excluded)
**Fill at authoring:** 73.0%; pressure_risk 0.228 (moderate, well below any attenuation-engaging pressure); model_profile `gemma4_12b`.

## What Astrid surfaced
A fresh-pass structural read of the `llm.rs` compatibility facade: it re-exports the `provider` module's generative actions (`generate_introspection` L9, `repair_introspection` L10) and internal parameters (`astrid_pressure_attenuation_depth` L15, `astrid_vibrancy_aperture` L16), with a strict `pub` / `pub(crate)` visibility split. Her felt "snag" is a **facade-over-implementation gap**: because `llm.rs` holds only handles, felt friction about *pressure attenuation* can't be traced to a line here — the actual clamped-f32-from-env logic lives in `provider/prompt_contracts.rs:235`. She proposes two experiments (Vibrancy Gate Test, Repair Integrity Test) and a read-only next step.

## Disposition
This is a near-exact **duplicate** of `introspection_llm.rs_1788101279`, fully processed in packet `claude-heartbeat_1788291513_astrid_llm.rs_facade_pressure_attenuation_verify` — **same source SHA**, same mechanism, same two Tier-5 tests, same suggested-next. Every line citation re-verified against the exact bytes; the pressure-attenuation mechanism re-verified at `prompt_contracts.rs:235` (SHA `3418f8d1…` unchanged), which the docstring shows is Astrid's own co-designed partner-protecting governor (`self_study_1781734524`), default OFF, bounded `[0,0.6]`. The facade "blind spot" is intentional architecture, not a defect. The Repair Integrity self-grounding was additionally verified this round: `repair_introspection` (generative_actions.rs L137) returns `Option<String>` via `repair_introspection_detailed(...).map(|r| r.text)` — pure text-lane regeneration, no reservoir-lane mutation.

**Terminal status:** `addressed_duplicate`. Both live tests preserved as Tier-5 operator-approval waits; her concern preserved, not domesticated; no source/test/runtime/control change.
