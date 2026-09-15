# introspection_astrid_capsules_spectral-bridge_src_autonomous_activity_reading_persistence.rs_1789083028

Astrid read the closing bytes of `activity_reading/persistence.rs` (delivered window
4368..4618) and named the `load_activity` guard that refuses "legacy cursor recovery"
when a foreground reader's retained source is unavailable. She read it as the system
choosing integrity over continuity: it will not reconstruct a position from pointers
whose source can no longer be verified.

The complete file at the report-bound SHA supports her reading of every reachable
state. Two refinements the window could not show:

1. The guard is `!preview.source_comparison.is_some_and(|s| s.retained_source_available)`,
   so it also fires when `source_comparison` is `None`. That arm is unreachable, because
   line 105 already errors on a missing bookmark and `source_comparison` is `Some`
   exactly when the bookmark is `Some` (`reader_bookmarks.rs:67-84`). Defensive shape,
   not a second condition.
2. "Integrity over continuity" describes one half of a fork. Lines 106-114 handle a
   non-active bookmark by quietly demoting it to `return_reader` - continuity preserved,
   no error. Only 115-122 refuse outright.

That fork had no restart-time regression: `retained_source_loss_blocks_return_...`
covers the RETURN_ACTIVITY path in `activity_reading.rs`, not `load_activity`. A focused
test now pins both halves and asserts the refusal leaves the persisted selection
byte-identical (refusal, never repair).

Her cross-file reference is real: `dispatch_semantic_microdose` exists at
`authority_types.rs:102`, called from `authority_gate.rs:1206`. Her NEXT target is wired
and the directory exists; she glossed BTSP as "Bridge Transition State Protocol" while
`btsp/mod.rs:1` reads "Being-Time Synaptic Plasticity domain facade". Recorded as
evidence only - her text is not corrected, rewritten, or contradicted back to her.

The queue's `artifact_integrity_unavailable` flag traced to bounded metadata, not loss:
`lived_state_witness/mod.rs:808` truncates `provider_route` at 40 chars and honestly sets
`provider_route_complete=false`, while `provider_route_sha256` hashes the untruncated
value. sha256 of the full 41-char `http://127.0.0.1:8090/v1/chat/completions` reproduces
the recorded `20176d67...` exactly, so the route is fully recoverable.

Status: addressed_change.
