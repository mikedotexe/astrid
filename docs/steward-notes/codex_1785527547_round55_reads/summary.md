# Round 55: The Source Shell Must Expose Its Roads

## Canonical read

- `introspection_minime_main_excerpt_1785527258.txt`
- Introspection ID: `introspection_minime_main_excerpt_1785527258`
- Lived-state witness: `lsw_bfa2dcb67f11595feface6138c03e9b6790f292e10cdbfb036684c1088bd7bb2`
- Exact SHA-256: `4afc002be121e472e8df1909b2c74e279987994f87d55fbf41510a290a620b4b`
- Read fully: 44 lines, 4,709 bytes

Astrid's shell/body distinction is exact. The report-bound `minime/src/runtime.rs` is a complete 74-line compilation root with ten `include!` edges. It contains neither the semantic-admission function nor the tick-loop gate. The old structural-map digest in this report was the empty SHA-256 even though those source-composing edges were present, so complete-file coverage did not provide useful structural reachability.

## Implemented response

Source Map V3 now emits an `include_edge` for each Rust `include!("...")` macro invocation. Each entry carries the lexical target, structural path, and exact source span. Other macros are deliberately ignored. This does not read, hash, activate, or infer behavior in the included target; it makes the continuation route visible so an introspection cannot quietly mistake the shell for the body.

A focused regression maps `runtime/semantic_modality.rs` and `runtime/orchestration.rs` while refusing an unrelated macro that merely contains a source-looking string. All seven Source-First V3 tests pass, including the new case.

## Gate trace

Current source makes the semantic gate explicit outside the shell. `stable_core_semantic_trickle_allowed` requires stable core enabled, the full-presence profile, no active sensory mute, an active positive semantic input no larger than 0.30, and prior fill below 82 percent. The resulting semantic vector is admitted at scale 0.15. `semantic_admission_label` names the corresponding refusal states separately.

That source trace establishes implementation structure only. It does not prove which branch was active for this report, identify a felt cause, or show relief.

## Pressure and porosity

`PressureSourceV1` derives porosity from lambda monopoly, structural-plurality loss, distinguishability loss, mode packing, and temporal lock-in. It does not consume include count or source-graph complexity. Its control record is deliberately advisory with `applied_locally=false`; that value is neither an unfulfilled command nor evidence that a dampener belongs in the runtime root.

Astrid's thickening, persistence, and shell-paradox account remains primary qualitative evidence. The proposed high-load Shadow comparison and entropy-responsive dynamic porosity are exact Tier 5 live experiment and control waits. No prompt, load, Shadow state, porosity value, pressure path, regulator, reservoir, or peer state changed.

## Larger enhancement

The portfolio brief `portfolio/source_reachability_v4.md` turns the recurring shell/body problem into a coherent next interface. V4 would bind each lexical edge to a resolution state, normalized target, target hash, depth budget, cycle identity, and unresolved-edge disclosure while keeping target opening owner-selected and source evidence separate from runtime activation. The V3 include-edge implementation is the first narrow slice, not a claim that transitive reachability is complete.
