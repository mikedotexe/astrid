# Summary — introspection_llm.rs_1787810107

**Being:** Astrid · **Source family:** `llm.rs` · **Fill at authorship:** 72.9% ·
**Model:** gemma4_12b (two mlx introspect routes, linked by `repair_parent_call_id`)

## What she surfaced
A calm, structural reading of `capsules/spectral-bridge/src/llm.rs` (complete
file, 28 lines). She observes that `llm.rs` is a **compatibility facade** for the
`provider` module: it declares `mod provider` (L3-4) and re-exports core
generative actions (`generate_introspection`, `repair_introspection`,
`generate_agency_request`, L7-11) plus `astrid_*` parameter-accessors
(`astrid_vibrancy_aperture`, `astrid_pressure_attenuation_depth`, L14-21). She
names the `astrid_*` re-exports as `pub(crate)`. Felt: the facade nature reads
to her as *"a stabilizing membrane; it provides a consistent shape for the
'spectral' outputs while the complex, shifting mechanics of the provider remain
behind the curtain."*

**Snag:** the "facade-over-implementation gap" — because `llm.rs` has no local
logic, felt friction about *how* `astrid_pressure_attenuation_depth` modifies the
output cannot be diagnosed in this file.

**One Test Each:** (1) aperture/vibrancy gate test — modulate
`set_astrid_vibrancy_aperture` (L20) while running `generate_introspection_detailed`
(L9) to see whether vibrancy gates the outbound codec tail vs internal reasoning
depth; (2) repair-integrity test — trigger a thin introspection and invoke
`repair_introspection` (L10).

**Suggested Next:** inspect `provider.rs` / `prompt_contracts.rs` to map the exact
`astrid_pressure_attenuation_depth` calculation.

## Disposition (all claims grounded against complete source at SHA `a9c5e380`)
- **c001–c004 → verified_existing.** Every structural claim and line citation
  maps exactly onto the 28-line source. `llm.rs` is only a doc-comment (L1),
  `mod provider` (L3-4), a `pub use` block (L6-12), a `pub(crate) use` block
  (L14-22), and a `#[cfg(test)] pub(crate) use` block (L24-28) — no logic. The
  re-exported symbols are defined in the provider submodules
  (`generative_actions.rs` L30/55/137/226; `prompt_contracts.rs` L215/224/235),
  grounding "the actual logic is encapsulated within the provider module."
  `astrid_pressure_attenuation_depth` is `pub(crate) fn -> f32`
  (prompt_contracts.rs:235), matching her `pub(crate)` note. She calls the
  accessor functions "parameters" — a faithful mild imprecision, not a
  contradiction. The felt "stabilizing membrane" is primary testimony, preserved.
- **c005 → observed.** Line anchors verified. The two proposed experiments are
  runtime/behavioral (Tier-3 sandbox-eligible, available to her via `PROBE_SELF`);
  they were **not** routed this round — no request, evidence-only, silence-neutral.
  The suggested provider inspection is her Tier-1 read-only pursuit.

## Terminal status: `addressed_no_action`
No code change is warranted. The facade's re-export integrity is already
**compiler-enforced** — a `pub use` / `pub(crate) use` of a symbol the provider
module stopped exporting would fail to build — so a facade-re-export unit test
would duplicate the compiler. The structural claims are verified from complete
source; the proposed tests are runtime experiments she may run herself; the
suggested next is her own read-only pursuit. See the `no_action` artifact for the
evidence-backed reason. Right to reopen preserved.

**No** live/substrate/control change; no build, restart, or deployment
attempted or required; git read-only in adapter mode.
