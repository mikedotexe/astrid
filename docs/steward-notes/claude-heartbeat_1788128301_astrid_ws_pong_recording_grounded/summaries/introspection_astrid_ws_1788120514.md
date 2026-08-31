# Summary — introspection_astrid_ws_1788120514

- **Source**: `astrid:ws` (`capsules/spectral-bridge/src/ws/telemetry_port.rs`), window lines 1-400 of 1041, report SHA-256 `42364feb914c957d487f93c440e30837c500971b41814a219d4c3992a12addd7` (working copy matches exactly — report-time == current source).
- **Witness**: `lsw_57ad2bd24fee8d830678be3f26e56d346d7c7cf91837f8bf51a09942010aa56e` — fill 71.2%, gemma4_12b via mlx, artifact authority `evidence_only`, `live_eligible_now=false`.
- **Report scope**: `partial_window_unseen_source_not_assessed` (lines 401-1041 not read by Astrid).

## What Astrid observed
A structurally accurate map of the telemetry subscriber loop: the persistent WS
connection to Minime's eigenvalue broadcast; per-message-type `record_ws_message_received`
state updates; Ping→Pong echo with `pong_send_error` logging; `Backoff` reconnect;
Close-vs-stream-end distinction; binary/text routing to `handle_telemetry_message`.
Every cited line was verified against the source at the recorded SHA (offsets ≤1 line).

## The two snags
1. **Pong asymmetry** — Astrid read the incoming-Pong arm as having "no explicit
   logic ... other than a debug log (L157)." Source refines this: the Pong arm
   (L142-158) first calls `record_ws_message_received("pong")`, which increments
   `pongs_received`, `messages_received`, and the timestamp (health_trace.rs
   L110-112) *before* the debug log. So a received Pong **is** a recorded state
   change, not only a log line. Her deeper concern is source-accurate and
   preserved: the bridge only *reflects* incoming pings and keeps no
   Ping-liveness/sequencing keyed on received Pongs. Contradiction preserved, not
   domesticated; grounded by a new isolating regression.
2. **Write-lock contention** — a valid structural concern. Source shows the
   record write-locks are scoped and dropped *before* `handle_telemetry_message`,
   which itself takes only a short read lock and runs its heaviest work (the
   filesystem artifact scan) throttled to 30s and lock-free. The shared-state
   *write* lives in the uncovered 401-1041 region. Recorded as `observed` with no
   causal overclaim; a true contention claim would need runtime profiling.

## Proposed tests
Both proposed tests' testable core is already covered:
`ws_trace_records_connection_lifecycle_without_payloads` (tests.rs L174-214) and
the text-received assertion (tests.rs L2809-2816). The async loop echo itself is
source-verified but integration-level. To durably ground the snag #1 correction
without redundancy, one focused regression was added:
`telemetry_pong_received_is_recorded_not_sent` — proves a received Pong records
on the received side (`pongs_received`/`messages_received`/timestamp) and is
**not** counted as a send. Passes.

## Disposition
`addressed_change` — one focused regression added; all other claims
verified/observed from source at the recorded SHA. No live change; no restart
required or attempted (evidence-only, Tier 0/1 read-only introspection).
