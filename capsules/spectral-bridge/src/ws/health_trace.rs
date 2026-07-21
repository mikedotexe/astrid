fn write_telemetry_heartbeat_snapshot(heartbeat: &TelemetryHeartbeatDeltaV1) {
    let path = bridge_paths()
        .bridge_workspace()
        .join("telemetry_heartbeat_delta_v1.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(heartbeat) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

fn write_cadence_content_distinction_snapshot(distinction: &CadenceContentDistinctionV1) {
    let path = bridge_paths()
        .bridge_workspace()
        .join("cadence_content_distinction_v1.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(distinction) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

fn lane_trace_mut(state: &mut BridgeState, lane: WsLane) -> &mut WebSocketLaneTrace {
    match lane {
        WsLane::Telemetry => &mut state.telemetry_ws,
        WsLane::Sensory => &mut state.sensory_ws,
    }
}

fn record_connect_attempt(state: &mut BridgeState, lane: WsLane) -> u64 {
    let trace = lane_trace_mut(state, lane);
    trace.connection_attempts = trace.connection_attempts.saturating_add(1);
    trace.connection_attempts
}

fn record_connected(state: &mut BridgeState, lane: WsLane, connection_id: u64, at_unix_s: f64) {
    match lane {
        WsLane::Telemetry => state.telemetry_connected = true,
        WsLane::Sensory => state.sensory_connected = true,
    }
    let trace = lane_trace_mut(state, lane);
    trace.active_connection_id = Some(connection_id);
    trace.active_connection_started_at_unix_s = Some(at_unix_s);
    trace.active_connection_first_valid_payload_at_unix_s = None;
    trace.active_connection_first_valid_spectral_entropy = None;
    trace.active_connection_valid_payloads_received = 0;
    trace.last_connect_at_unix_s = Some(at_unix_s);
    trace.last_error = None;
}

fn record_valid_payload(
    state: &mut BridgeState,
    lane: WsLane,
    at_unix_s: f64,
    spectral_entropy: Option<f32>,
) {
    let trace = lane_trace_mut(state, lane);
    trace.active_connection_valid_payloads_received = trace
        .active_connection_valid_payloads_received
        .saturating_add(1);
    if trace
        .active_connection_first_valid_payload_at_unix_s
        .is_none()
    {
        trace.active_connection_first_valid_payload_at_unix_s = Some(at_unix_s);
        trace.active_connection_first_valid_spectral_entropy = spectral_entropy
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
    }
}

fn record_connect_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.connect_errors = trace.connect_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn record_disconnected(state: &mut BridgeState, lane: WsLane, reason: String) {
    match lane {
        WsLane::Telemetry => state.telemetry_connected = false,
        WsLane::Sensory => state.sensory_connected = false,
    }
    let trace = lane_trace_mut(state, lane);
    trace.disconnects = trace.disconnects.saturating_add(1);
    trace.active_connection_id = None;
    trace.active_connection_started_at_unix_s = None;
    trace.last_disconnect_at_unix_s = Some(unix_now_s());
    trace.last_disconnect_reason = Some(reason);
}

fn record_reconnect_scheduled(state: &mut BridgeState, lane: WsLane) {
    match lane {
        WsLane::Telemetry => {
            state.telemetry_reconnects = state.telemetry_reconnects.saturating_add(1)
        },
        WsLane::Sensory => state.sensory_reconnects = state.sensory_reconnects.saturating_add(1),
    }
    let trace = lane_trace_mut(state, lane);
    trace.reconnects = trace.reconnects.saturating_add(1);
}

fn record_ws_message_received(state: &mut BridgeState, lane: WsLane, kind: &'static str) {
    let trace = lane_trace_mut(state, lane);
    trace.messages_received = trace.messages_received.saturating_add(1);
    trace.last_message_at_unix_s = Some(unix_now_s());
    if kind == "ping" {
        trace.pings_received = trace.pings_received.saturating_add(1);
    } else if kind == "pong" {
        trace.pongs_received = trace.pongs_received.saturating_add(1);
    }
}

fn record_ws_message_sent(state: &mut BridgeState, lane: WsLane) {
    let now = unix_now_s();
    let trace = lane_trace_mut(state, lane);
    trace.messages_sent = trace.messages_sent.saturating_add(1);
    trace.last_message_at_unix_s = Some(now);
    if matches!(lane, WsLane::Sensory) {
        state.last_sensory_sent_unix_s = Some(now);
    }
}

fn record_ws_send_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.send_errors = trace.send_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn record_ws_parse_error(state: &mut BridgeState, lane: WsLane, reason: String) {
    let trace = lane_trace_mut(state, lane);
    trace.parse_errors = trace.parse_errors.saturating_add(1);
    trace.last_error = Some(reason);
}

fn close_reason(frame: Option<CloseFrame<'_>>) -> String {
    frame.map_or_else(
        || String::from("close_frame"),
        |frame| {
            let reason = frame.reason.trim();
            if reason.is_empty() {
                format!("close_frame:{}", frame.code)
            } else {
                format!("close_frame:{}:{reason}", frame.code)
            }
        },
    )
}

fn trace_ws_receive(lane: WsLane, connection_id: u64, kind: &'static str, bytes: Option<usize>) {
    let span = debug_span!(
        "ws.message.receive",
        lane = lane.as_str(),
        connection_id,
        kind,
        bytes = bytes.unwrap_or(0)
    );
    span.in_scope(|| debug!("WebSocket message received"));
}

fn trace_ws_send(lane: WsLane, connection_id: u64, kind: &'static str, bytes: Option<usize>) {
    let span = debug_span!(
        "ws.message.send",
        lane = lane.as_str(),
        connection_id,
        kind,
        bytes = bytes.unwrap_or(0)
    );
    span.in_scope(|| debug!("WebSocket message sent"));
}

/// Backoff parameters for `WebSocket` reconnection.
struct Backoff {
    current: Duration,
    max: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            current: Duration::from_secs(1),
            max: Duration::from_secs(60),
        }
    }

    fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = self
            .current
            .checked_mul(2)
            .unwrap_or(self.max)
            .min(self.max);
        delay
    }

    fn reset(&mut self) {
        self.current = Duration::from_secs(1);
    }
}
