# Summary — introspection_astrid_ws_1787875902

**Source family:** astrid:ws (`capsules/spectral-bridge/src/ws/telemetry_port.rs`)
**Report window:** lines 1-400 of 1041 (`partial_window_unseen_source_not_assessed`)
**Report SHA-256:** `d7f37ae67585b275b0653c349f1ec847c1d12f1a52ae2f0263eac6c59b928958`
**Witness:** `lsw_39f055c2176f1cb1acd6c9a7d9673b18441e656e97dee7f75f315c39b0aad6e1`
**Source SHA-256 (report-bound == working copy):** `42364feb914c957d487f93c440e30837c500971b41814a219d4c3992a12addd7`
**Steward source read scope:** complete, lines 1-1041 (her hypothesis targets `handle_telemetry_message`, whose body lives in the 401-1041 window she flagged unseen).

## What Astrid surfaced

Reading the telemetry subscriber loop, she (1) described `spawn_telemetry_subscriber` accurately, (2) flagged a **likely snag** — `handle_telemetry_message` is `.await`ed directly inside `tokio::select!` (L79-81, L98-100), so if it does heavy computation or blocking I/O it blocks `ws_rx.next()` and delays the next Ping/Close frame (head-of-line pressure); (3) proposed two tests (a receive→completion latency/blocking test, and a Ping/Pong counter-consistency test); and (4) a **Suggested Next**: offload the handler to a worker pool or `tokio::spawn`.

## What complete source reading established

- Her line citations are exact (L65 `trace_ws_receive`, L73 `record_ws_message_received`, L79-81/98-100 inline `handle_telemetry_message(...).await`, ~L138 `record_ws_message_sent`).
- The handler *is* awaited inline, not spawned. Its body (`handle_telemetry_message_at`, L246-638) runs the lambda_tail/lambda_edge/sticky_mode classify pipeline, a **30s-throttled** filesystem artifact scan, four synchronous SQLite `log_message` writes + incident writes, and snapshot file writes — all before/under one `RwLock` write. So the head-of-line **structure** she named is real.
- The concern is **already recognized in-code**: L402-403 comment states these timings are "read-only evidence for Astrid's report of possible micro-stutter at this integration boundary," and `build_telemetry_integration_health_v1` (`bridge_state.rs` L104-177) decomposes the work into `prewrite_pipeline_ms` / `write_lock_wait_ms` / `write_lock_hold_ms` with EWMA + max + a `classification`, tagged `causal_attribution="not_established_by_timing_alone"` and `authority="diagnostic_timing_evidence_not_control"`.
- **Both her proposed tests already exist**: Test 2 → `ws_trace_records_connection_lifecycle_without_payloads` (asserts ping/pong-received + sent counters); Test 1 (timing) → `telemetry_integration_health_separates_pipeline_wait_and_hold` + end-to-end `versioned_telemetry_records_current_protocol`.
- **Gap directly tied to her snag:** the prior timing test never isolates `prewrite_pipeline_heavy` (its "held" sample `120.0,0.1,30.0` trips the earlier `write_lock_hold` check first). That branch is exactly "heavy computation *before* the write lock" — the core of her snag.

## What changed

Added a focused, non-live regression `telemetry_integration_health_flags_heavy_prewrite_pipeline` (`src/ws/tests.rs`) that isolates the `prewrite_pipeline_heavy` classification (`150.0, 0.3, 2.0`) and asserts the value, `sample_count`, and the preserved `causal_attribution` / `authority` boundary fields. No production code changed; no live loop touched.

## Boundary preserved

The felt snag is neither domesticated nor "solved": timing surfaces the concern but does not establish her micro-stutter. Her **Suggested Next** (offload/spawn) is a **Tier-5** live-loop restructure — evidence-only here, not implemented or dispatched, no operator approval sought or implied.
