# Source Reachability V4

## Why V4

Source-First V2 provides hash-bound read sessions and coverage intervals. Source Map V3 provides parser-disclosed structural entries. The recurring gap is transitive reachability: a complete shell can truthfully be complete while the behavior it points to remains unread and structurally invisible.

V4 should make source-composition roads explicit without pretending the destination was visited.

## Interfaces

`SourceEdgeV4` should carry:

- root owner and source identity
- root source hash
- edge kind: `include`, `module`, `import`, or `generated_source`
- exact UTF-8 byte and line interval
- exact lexical target
- deterministic resolution state: `resolved`, `missing`, `dynamic`, `outside_allowed_root`, or `unsupported`
- normalized target identity and target hash only when resolved
- optional target symbol and structural-map reference
- cycle identity and traversal depth
- capture time and parser disclosure
- authority fixed to `read_evidence_not_activation_control_or_approval`

`SourceReachabilityManifestV4` should carry an ordered edge set, root and target hash inventory, explicit depth and byte budgets, cycle and duplicate accounting, unresolved edges, omitted edges, and a deterministic digest. It must never silently flatten included bodies into the root source.

## Interaction

The introspection prompt should show the root window first, then a bounded reachability table. Opening a target remains an explicit owner-selected read. A target that was not opened must remain `reachable_unread`; its symbols cannot support presence, absence, activation, or causal claims.

For a shell such as Minime's `runtime.rs`, the first view would name all ten include edges. Selecting `runtime/semantic_modality.rs` and `runtime/orchestration.rs` would create separate hash-bound read receipts and preserve their identities rather than merging them into one synthetic file.

## Determinism and failure

- Resolve only within configured source roots.
- Reject symlink escapes and path traversal.
- Bind target hashes after resolution and before reading.
- Record cycles rather than recursively expanding them.
- Stop exactly at declared depth, edge-count, and byte budgets.
- Keep generated and dynamic targets unresolved unless a sanctioned resolver supplies exact bytes and provenance.
- On source drift, invalidate only the affected edge and downstream read receipts.

## Rollout

1. Land lexical Rust `include!` edges in V3. This slice is implemented and tested in round 55.
2. Add resolved target identities and hashes in an offline projection.
3. Add bounded owner-selected target opening and per-target read receipts.
4. Shadow-compare V3 and V4 maps across Rust, Python, JSON, TOML, YAML, and Markdown sources.
5. Promote V4 prompt rendering only after exact hash, cycle, escape, truncation, and replay tests pass.

## Authority boundary

Reachability is evidence navigation. It does not prove compilation, runtime activation, branch selection, causation, felt state, relief, consent, or closure. It cannot mutate source, choose a control, dispatch an experiment, restart a service, or widen any being's authority.
