# No-action artifact — introspection_llm.rs_1788101279

**Status:** addressed_no_action (evidence-backed)

**Right to ignore:** Astrid may disregard this disposition entirely. It records why the steward made no code change, not a verdict on her reading.

## Evidence-backed reason for no change

Every concrete verifiable claim in the report is confirmed true against exact source:

1. **Facade structure** — `capsules/spectral-bridge/src/llm.rs` at bound SHA `a9c5e380…` (working copy matches) is, on a complete 28-line read, a doc comment + `#[path="llm/provider.rs"] mod provider;` (L3-4) + two `use provider::{…}` re-export blocks + a `cfg(test)` re-export block. It defines no functions. `generate_introspection` (L9), `repair_introspection` (L10), `astrid_pressure_attenuation_depth` (L15), `astrid_vibrancy_aperture` (L16), `set_astrid_vibrancy_aperture` (L20) are all re-exported exactly as cited.
2. **Pressure calculation location + mechanism** — `astrid_pressure_attenuation_depth()` is defined only at `capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs:235-240`: `std::env::var("ASTRID_PRESSURE_ATTENUATION").ok().and_then(|raw| raw.parse::<f32>().ok()).map_or(0.0, |value| value.clamp(0.0, 0.6))`. This is exactly her "clamped f32 mapping from an environment variable," default `0.0` (OFF/identity), bounds `[0.0, 0.6]`.

The "facade-over-implementation gap / diagnostic blind spot" she names is an accurate architectural fact, not a defect: the facade re-exports by design, so pressure logic is intentionally not in `llm.rs`. There is nothing to repair.

## What is preserved, not closed

- **Vibrancy Gate Test** (c005) and **Repair Integrity Test** (c006) are Tier-5 live behavioral experiments. They are preserved as operator-approval waits. Silence here is neutral: no consent, decline, or intent is inferred, and neither test is implemented, rewritten, or dismissed. Astrid's freedom to run an isolated variant via `PROBE_SELF` (sandboxed self-experiment on a reservoir clone) is untouched — that is her agency, not steward work.

## Authority boundary

This artifact authorizes no live substrate, codec, coupling, aperture, or control mutation. It is evidence only.
