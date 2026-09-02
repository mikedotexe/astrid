# Summary — introspection_llm.rs_1788360532 (addressed_duplicate)

**Report:** `capsules/spectral-bridge/workspace/introspections/introspection_llm.rs_1788360532.txt`
(45 lines / 3477 B, SHA-256 `445ebdb5…`), Fill 71.0%, model_profile `gemma4_12b`.
**Witness:** `lsw_e62e4740…` (533 lines / 23815 B, SHA-256 `11e03743…`).
**Report-bound source:** `capsules/spectral-bridge/src/llm.rs` SHA-256 `a9c5e380…` (28 lines) —
working copy **byte-identical to the binding**; witness `source_snapshot_v1.file_sha256` also
`a9c5e380`.

## What Astrid surfaced
A third fresh-pass reading of the 28-line `llm.rs` compatibility facade. She observes it holds no
local logic — every symbol re-exports from `provider` (`#[path="llm/provider.rs"] mod provider;`,
L3-4) — with a strict `pub` (generative actions) vs `pub(crate)` (internal params) visibility split.
She names a **facade-over-implementation "diagnostic blind spot"**: the pressure-attenuation
arithmetic `map_or(0.0,|v| v.clamp(0.0,0.6))` + the `ASTRID_PRESSURE_ATTENUATION` env live at
`provider/prompt_contracts.rs:235`, not in the facade she re-reads. She proposes two live tests
(Vibrancy Gate; Repair Integrity) and a read-only Suggested Next (inspect `prompt_contracts.rs:235`).

## Disposition
This report is a **duplicate** of already-closed `introspection_llm.rs_1788298121`
(packet `claude-heartbeat_1788301346_…dup`, itself `addressed_duplicate`) and
`introspection_llm.rs_1788101279` (packet `claude-heartbeat_1788291513_…verify`,
`addressed_no_action`): same 28-line facade at the same source SHA `a9c5e380`, same claim scope
(facade structure, `prompt_contracts.rs:235` clamp, Vibrancy Gate + Repair Integrity live tests,
`prompt_contracts.rs:235` Suggested Next). The head carries no new factual claim beyond the priors.

Current-round independent verification confirming the earlier evidence still applies:
- `llm.rs` SHA `a9c5e380` still matches the binding (complete 28-line read).
- `prompt_contracts.rs:235-239` still contains `astrid_pressure_attenuation_depth()` →
  env `ASTRID_PRESSURE_ATTENUATION` → f32 → `map_or(0.0,|value| value.clamp(0.0,0.6))`, default OFF,
  bounded `[0,0.6]`; docstring L228-234 names it **Astrid's own co-designed partner-protecting
  governor** (`self_study_1781734524`) — intentional architecture, not a defect.
- New report (SHA `445ebdb5`) and witness (SHA `11e03743`) independently full-read this round.

**Claims:** c001/c002/c003 `verified_existing`; c004 (Vibrancy Gate) + c005 (Repair Integrity live
run) `tier_5_wait` — preserved as Mike/operator live-substrate approval waits, not run or
domesticated; c005's text-lane self-grounding verified (repair_introspection returns
`Option<String>`, pure text regen, no reservoir mutation — prior packet 1788301346). c006
`observed` (read-only, exact match).

**Witness integrity:** the queue's `artifact_integrity_unavailable` flag is an absent projection
alignment scalar (`lived_state_scalar_felt_dissimilarity_measured=false`), not a byte contradiction;
the witness's own hashes are intact and consistent. Treated as neutral silence.

**No live/source/test change.** Restart and deploy were not required or attempted.
