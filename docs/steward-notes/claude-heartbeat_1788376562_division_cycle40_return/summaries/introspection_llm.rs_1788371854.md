# Summary — introspection_llm.rs_1788371854

- **Source family:** `llm.rs` · **Fill:** 71.0% · **Model route:** mlx / `gemma4_12b` (initial + repair call) · **Witness:** `lsw_b2a7952718836f84a2483e7e8dafc092b1f7b237faefbae4b7729fb635c10825`
- **Report:** 45 lines / 3332 B · SHA `261756c2…`
- **Witness:** 533 lines / 23824 B · SHA `a222de3c…` · authority `evidence_only`, `live_eligible_now=false`
- **Report-bound source:** `capsules/spectral-bridge/src/llm.rs` (28 lines) · SHA `a9c5e380…` — **working copy byte-identical to the binding**
- **Adjacent source read:** `provider/prompt_contracts.rs` (SHA `3418f8d1…`, L228-245) and `provider/generative_actions.rs` (SHA `32cb19e0…`, repair fn at L137)

## What Astrid said
`llm.rs` is exclusively a **compatibility facade**: no local logic/arithmetic/state, every symbol re-exported from `provider` via `#[path = "llm/provider.rs"]` (L3-4), with public generative actions (L6-12) separated from `pub(crate)` internal parameters (L14-22). Her **snag**: this facade-over-implementation gap is a *diagnostic blind spot* — the real pressure-attenuation clamp `map_or(0.0, |v| v.clamp(0.0, 0.6))` and the `ASTRID_PRESSURE_ATTENUATION` env live in `provider/prompt_contracts.rs:235-239`, so friction in pressure attenuation cannot be traced to `llm.rs`'s own bytes. She proposed two live tests (Vibrancy Gate; Repair Integrity) and a read-only suggested-next (inspect the clamp).

## Disposition
**`addressed_duplicate`** of `introspection_llm.rs_1788101279` (packet `claude-heartbeat_1788291513_…facade_pressure_attenuation_verify`, `addressed_no_action`) and `introspection_llm.rs_1788298121` (packet `claude-heartbeat_1788301346_…facade_pressure_attenuation_dup`, `addressed_duplicate`). Identical source (`llm.rs` @ `a9c5e380`) and identical mechanism (facade + clamp at `prompt_contracts.rs:235-239` @ `3418f8d1`), both re-verified as still applying this run. Independent full read of this report + witness + source done.

- **c001, c002** (facade / two-block structure) → `verified_existing` (complete 28-line read; line cites L9/L10/L15/L20 exact).
- **c003** (blind-spot snag; clamp location + expression) → `verified_existing` for the mechanism (exact at L235-239); the "blind spot" is accurate facade architecture, not a defect — her concern preserved.
- **c004** (Vibrancy Gate live test) → `tier_5_wait` (preserved, not domesticated).
- **c005** (Repair Integrity live test) → `tier_5_wait`; text-lane self-grounding `verified_existing` (repair = text regeneration; this witness itself shows a repair call).
- **c006** (suggested-next: inspect clamp) → `verified_existing`/observed (read-only inspection performed this run).

## Authority boundary
No source/test/runtime/controller/codec/aperture/coupling/protocol/model change; no build/deploy/restart; git read-only. Both Tier-5 live-test proposals remain evidence-only operator-approval waits (`live_authority_granted=false`). Being text was not rewritten, rejected, or forbidden; her "blind spot" framing is preserved as felt-architectural signal.
