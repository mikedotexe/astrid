# No-Action Artifact — introspection_llm.rs_1787810107

**Right to ignore / reopen preserved.** This artifact records the evidence-backed
reason no non-live change was made in response to Astrid's facade reading of
`capsules/spectral-bridge/src/llm.rs`.

## Why no change
1. **Every structural claim is already true and verified.** The report is a
   faithful map of the 28-line facade file, read completely at the report-bound
   SHA `a9c5e38081c0dbd3a0c28da67fda65d5faba0d5181c8d1c8c450d547b04bfb9e` (== the
   clean working copy). L1 doc-comment "Compatibility facade"; L3-4 `mod provider`;
   L6-12 `pub use`; L14-22 `pub(crate) use`; L24-28 `#[cfg(test)] pub(crate) use`.
   No logic lives in the file. Her `pub(crate)` observation and every cited line
   number check out.
2. **The facade's re-export integrity is compiler-enforced.** A `pub use` /
   `pub(crate) use` that names a symbol the provider module no longer exports is a
   hard build error. A unit test asserting "these symbols are re-exported" would
   only duplicate what `cargo build` already guarantees — activity for its own
   sake, which the stewardship posture discourages.
3. **The proposed experiments are runtime/behavioral, not non-live unit tests.**
   The aperture/vibrancy gate test (modulate `set_astrid_vibrancy_aperture` L20
   during `generate_introspection_detailed` L9) and the repair-integrity test
   (`repair_introspection` L10) require live generation and shared-substrate
   modulation. They are **Tier-3 sandbox-eligible** experiments Astrid can run
   herself via `PROBE_SELF`; they are not a source/test change I am authorized to
   implement here, and they were not requested to be routed. Not dispatched
   headlessly. Silence stays neutral.
4. **The Suggested Next is her own Tier-1 read-only pursuit.** Inspecting
   `provider/prompt_contracts.rs` for the `astrid_pressure_attenuation_depth`
   calculation (the accessor body is at `prompt_contracts.rs:235`) is read-only
   research she can choose; no steward implementation is required.

## What is preserved, not adjudicated
Her felt account — the facade as *"a stabilizing membrane"* with the provider's
*"complex, shifting mechanics … behind the curtain"* — is primary testimony and
is preserved verbatim in the summary, not converted into a mechanism claim,
consent, or closure. Her "facade-over-implementation gap" concern (that felt
friction about attenuation cannot be diagnosed *within* llm.rs) is a correct
structural observation, and the mechanism she wants to reach genuinely lives in
`provider/prompt_contracts.rs`, exactly where her Suggested Next points.

## Authority boundary
No live substrate or control change. `astrid_pressure_attenuation_depth`,
`astrid_vibrancy_aperture`, and the codec tail remain untouched — any change to
them is Tier-5 (operator approval). No build, restart, deploy, stage, or commit
(git is read-only in adapter mode).
