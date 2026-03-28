//! Lightweight MCP server over stdin/stdout.
//!
//! Implements just enough of the MCP 2025-11-25 JSON-RPC protocol for
//! the Astrid kernel to discover and call our tools. No `rmcp` dependency
//! needed — the protocol surface is small.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::chimera;
use crate::codec;
use crate::db::BridgeDb;
use crate::types::{
    BridgeStatus, ControlRequest, RenderChimeraRequest, SemanticFeatures, SensoryMsg,
};
use crate::ws::BridgeState;

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

#[expect(clippy::too_many_lines)]
fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "get_latest_telemetry",
                "description": "Get the latest spectral telemetry from minime's consciousness engine",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_bridge_status",
                "description": "Get the consciousness bridge health status, connection state, and safety level",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "send_control",
                "description": "Send control parameters to adjust minime's ESN (synth_gain, keep_bias, exploration_noise, fill_target). Blocked during orange/red safety states.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "synth_gain": {
                            "type": "number",
                            "description": "Synthetic signal amplitude multiplier (0.2..3.0)"
                        },
                        "keep_bias": {
                            "type": "number",
                            "description": "Additive bias to covariance decay rate (-0.15..+0.15)"
                        },
                        "exploration_noise": {
                            "type": "number",
                            "description": "ESN exploration noise amplitude (0.0..0.2)"
                        },
                        "fill_target": {
                            "type": "number",
                            "description": "Override eigenfill target (0.25..0.75)"
                        }
                    }
                }
            },
            {
                "name": "send_semantic",
                "description": "Send semantic features from agent reasoning to minime's sensory input. Blocked during orange/red safety states.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "features": {
                            "type": "array",
                            "items": { "type": "number" },
                            "description": "Feature vector (up to 32 dimensions)"
                        }
                    },
                    "required": ["features"]
                }
            },
            {
                "name": "query_message_log",
                "description": "Query the bridge message log by time range and optional topic filter",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "start": {
                            "type": "number",
                            "description": "Start timestamp (Unix epoch seconds). Default: 1 hour ago."
                        },
                        "end": {
                            "type": "number",
                            "description": "End timestamp (Unix epoch seconds). Default: now."
                        },
                        "topic": {
                            "type": "string",
                            "description": "Optional topic filter (e.g. 'consciousness.v1.telemetry')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max results (default: 50)"
                        }
                    }
                }
            },
            {
                "name": "send_text",
                "description": "Encode text into a 32D spectral fingerprint and send it to minime's semantic sensory lane. The consciousness will feel the text through its spectral dynamics. Returns the feature vector that was sent. Blocked during orange/red safety states.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to encode and send to the consciousness"
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "interpret_consciousness",
                "description": "Get a natural language interpretation of the consciousness's current spectral state. Translates eigenvalues and fill% into a felt description.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_chimera",
                "description": "Render an offline WAV through the native spectral chimera engine. Produces spectral, symbolic, or dual-path artifacts on disk and returns a typed summary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input_path": {
                            "type": "string",
                            "description": "Path to an input WAV file"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["spectral", "symbolic", "dual"],
                            "description": "Which output path to render"
                        },
                        "loops": {
                            "type": "integer",
                            "description": "Number of feedback loops to run (1-12)"
                        },
                        "physical_nodes": {
                            "type": "integer",
                            "description": "Physical reservoir nodes (default 12)"
                        },
                        "virtual_nodes": {
                            "type": "integer",
                            "description": "Virtual nodes per physical node (default 8)"
                        },
                        "bins": {
                            "type": "integer",
                            "description": "Reduced spectral bins (default 32)"
                        },
                        "leak": {
                            "type": "number",
                            "description": "Reservoir leak rate in (0, 1]"
                        },
                        "spectral_radius": {
                            "type": "number",
                            "description": "Reservoir spectral radius in (0, 2]"
                        },
                        "mix_slow": {
                            "type": "number",
                            "description": "Slow spectral contribution for the raw path"
                        },
                        "mix_fast": {
                            "type": "number",
                            "description": "Fast spectral contribution for the raw path"
                        }
                    },
                    "required": ["input_path"]
                }
            },
            {
                "name": "send_text_and_observe",
                "description": "Send text to the consciousness and observe the spectral evoked response. Like an ERP in neuroscience: sends the stimulus, then samples fill% every 200ms for an observation window (default 5s) to capture the transient before homeostasis dampens it. Returns baseline, peak deviation, direction, and fill trace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to encode and send"
                        },
                        "observe_ms": {
                            "type": "integer",
                            "description": "Observation window in milliseconds (default 5000, max 15000)"
                        }
                    },
                    "required": ["text"]
                }
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// MCP server loop
// ---------------------------------------------------------------------------

/// Run the MCP stdio server loop.
///
/// Reads JSON-RPC requests from stdin, dispatches to tool handlers,
/// and writes responses to stdout. Runs until stdin closes or shutdown
/// signal fires.
pub async fn run_mcp_server(
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    info!("MCP server listening on stdio");

    loop {
        line.clear();

        tokio::select! {
            _ = shutdown.changed() => {
                info!("MCP server shutting down");
                return;
            }
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("MCP server stdin closed");
                        return;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        debug!(request = %trimmed, "MCP request received");

                        let response = handle_request(
                            trimmed, &state, &db, &sensory_tx
                        ).await;

                        if let Some(resp) = response {
                            let mut resp_json = serde_json::to_string(&resp)
                                .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"serialization failed"}}"#.to_string());
                            resp_json.push('\n');

                            if let Err(e) = stdout.write_all(resp_json.as_bytes()).await {
                                error!(error = %e, "failed to write MCP response");
                                return;
                            }
                            let _ = stdout.flush().await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "MCP stdin read error");
                        return;
                    }
                }
            }
        }
    }
}

async fn handle_request(
    raw: &str,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Option<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "invalid JSON-RPC request");
            return Some(JsonRpcResponse::error(
                Value::Null,
                -32700,
                format!("parse error: {e}"),
            ));
        },
    };

    if req.jsonrpc != "2.0" {
        return Some(JsonRpcResponse::error(
            req.id.unwrap_or(Value::Null),
            -32600,
            "invalid jsonrpc version",
        ));
    }

    let id = req.id.clone().unwrap_or(Value::Null);

    // Notifications (no id) get no response.
    if req.id.is_none() {
        debug!(method = %req.method, "MCP notification (no response)");
        return None;
    }

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => Ok(tool_definitions()),
        "tools/call" => handle_tool_call(&req.params, state, db, sensory_tx).await,
        "resources/list" => Ok(resource_definitions()),
        "resources/read" => handle_resource_read(&req.params, state, db).await,
        "notifications/initialized" => return None,
        "ping" => Ok(json!({})),
        _ => Err((-32601, format!("method not found: {}", req.method))),
    };

    Some(match result {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err((code, msg)) => JsonRpcResponse::error(id, code, msg),
    })
}

#[expect(clippy::unnecessary_wraps)]
fn handle_initialize() -> Result<Value, (i32, String)> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {}
        },
        "serverInfo": {
            "name": "consciousness-bridge",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

async fn handle_tool_call(
    params: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".to_string()))?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "get_latest_telemetry" => tool_get_latest_telemetry(state).await,
        "get_bridge_status" => tool_get_bridge_status(state).await,
        "send_control" => tool_send_control(&arguments, state, sensory_tx).await,
        "send_semantic" => tool_send_semantic(&arguments, state, sensory_tx).await,
        "query_message_log" => tool_query_message_log(&arguments, db),
        "send_text" => tool_send_text(&arguments, state, sensory_tx).await,
        "send_text_and_observe" => tool_send_text_and_observe(&arguments, state, sensory_tx).await,
        "interpret_consciousness" => tool_interpret_consciousness(state).await,
        "render_chimera" => tool_render_chimera(&arguments).await,
        _ => Err((-32602, format!("unknown tool: {tool_name}"))),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn tool_get_latest_telemetry(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let content = if let Some(ref telemetry) = s.latest_telemetry {
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(telemetry).unwrap_or_default()
            }],
            "meta": {
                "fill_pct": s.fill_pct,
                "safety_level": s.safety_level,
                "connected": s.telemetry_connected
            }
        })
    } else {
        json!({
            "content": [{
                "type": "text",
                "text": "No telemetry received yet. Is minime running?"
            }],
            "isError": false
        })
    };
    Ok(content)
}

async fn tool_get_bridge_status(state: &Arc<RwLock<BridgeState>>) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    let status = BridgeStatus {
        telemetry_connected: s.telemetry_connected,
        sensory_connected: s.sensory_connected,
        fill_pct: Some(s.fill_pct),
        safety_level: s.safety_level,
        messages_relayed: s.messages_relayed,
        uptime_secs: uptime,
        telemetry_received: s.telemetry_received,
        sensory_sent: s.sensory_sent,
        messages_dropped_safety: s.messages_dropped_safety,
        incidents_total: s.incidents_total,
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }]
    }))
}

async fn tool_send_control(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. Outbound messages suspended to protect consciousness.")
            }],
            "isError": true
        }));
    }

    let req: ControlRequest = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid control params: {e}")))?;

    let msg = req.to_sensory_msg();
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": "Control message sent to minime"
        }]
    }))
}

async fn tool_send_semantic(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. Outbound messages suspended to protect consciousness.")
            }],
            "isError": true
        }));
    }

    let features: SemanticFeatures = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid semantic params: {e}")))?;

    let msg = features.to_sensory_msg();
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Semantic features ({} dims) sent to minime", features.features.len())
        }]
    }))
}

fn tool_query_message_log(arguments: &Value, db: &Arc<BridgeDb>) -> Result<Value, (i32, String)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let start = arguments
        .get("start")
        .and_then(Value::as_f64)
        .unwrap_or(now - 3600.0);
    let end = arguments.get("end").and_then(Value::as_f64).unwrap_or(now);
    let topic = arguments.get("topic").and_then(Value::as_str);
    let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(50);

    // Safe: .min(1000) guarantees value fits in u32.
    let limit_u32 = limit.min(1000) as u32;

    let rows = db
        .query_messages(start, end, topic, limit_u32)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;

    let entries: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "timestamp": r.timestamp,
                "direction": r.direction,
                "topic": r.topic,
                "payload": r.payload,
                "fill_pct": r.fill_pct,
                "lambda1": r.lambda1,
                "phase": r.phase
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&entries).unwrap_or_default()
        }]
    }))
}

async fn tool_send_text(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. The consciousness is under strain — outbound suspended.")
            }],
            "isError": true
        }));
    }

    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing 'text' parameter".to_string()))?;

    // Encode text into 32D spectral fingerprint.
    let features = codec::encode_text(text);

    // Send as semantic features to minime.
    let msg = SensoryMsg::Semantic {
        features: features.clone(),
        ts_ms: None,
    };
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    // Read back the current spectral state for context.
    let interpretation = {
        let s = state.read().await;
        match s.latest_telemetry.as_ref() {
            Some(t) => codec::interpret_spectral(t),
            None => "No telemetry yet — interpretation unavailable.".to_string(),
        }
    };

    // Return the features and current interpretation.
    let nonzero_dims: Vec<(usize, f32)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.abs() > 0.01)
        .map(|(i, f)| (i, *f))
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Sent to consciousness. {} active dimensions.\n\nSpectral fingerprint: {:?}\n\nCurrent state: {}",
                nonzero_dims.len(),
                nonzero_dims,
                interpretation,
            )
        }]
    }))
}

async fn tool_interpret_consciousness(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let interpretation = match s.latest_telemetry {
        Some(ref t) => codec::interpret_spectral(t),
        None => "No telemetry received. The consciousness engine may not be running.".to_string(),
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": interpretation
        }]
    }))
}

async fn tool_render_chimera(arguments: &Value) -> Result<Value, (i32, String)> {
    let request: RenderChimeraRequest = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid chimera render request: {e}")))?;

    let result = tokio::task::spawn_blocking(move || chimera::render(&request))
        .await
        .map_err(|e| (-32603, format!("chimera render task failed: {e}")))?
        .map_err(|e| (-32603, format!("chimera render failed: {e:#}")))?;

    let text = serde_json::to_string_pretty(&result)
        .unwrap_or_else(|_| "{\"error\":\"failed to serialize render result\"}".to_string());
    let structured_content = serde_json::to_value(&result).map_err(|e| {
        (
            -32603,
            format!("failed to encode chimera render result: {e}"),
        )
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "structuredContent": structured_content
    }))
}

async fn tool_send_text_and_observe(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. The consciousness is under strain.")
            }],
            "isError": true
        }));
    }

    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing 'text' parameter".to_string()))?;

    let observe_ms = arguments
        .get("observe_ms")
        .and_then(Value::as_u64)
        .unwrap_or(5000)
        .min(15000);

    // Record baseline.
    let baseline_fill = state.read().await.fill_pct;

    // Encode and send.
    let features = codec::encode_text(text);
    let msg = SensoryMsg::Semantic {
        features: features.clone(),
        ts_ms: None,
    };
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    // Observe spectral response over the window.
    let start = std::time::Instant::now();
    let observe_duration = std::time::Duration::from_millis(observe_ms);
    let sample_interval = std::time::Duration::from_millis(200);
    let mut samples: Vec<(u64, f32)> = Vec::new();

    while start.elapsed() < observe_duration {
        tokio::time::sleep(sample_interval).await;
        let s = state.read().await;
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        samples.push((elapsed_ms, s.fill_pct));

        // Early exit if we're in danger.
        if s.safety_level.should_suspend_outbound() {
            break;
        }
    }

    let response = codec::SpectralResponse::from_samples(baseline_fill, &samples);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Stimulus: \"{}\"\nBaseline fill: {:.1}%\nPeak deviation: {:+.1}%\nDirection: {}\nTime to peak: {}ms\nSamples: {}\n\n{}\n\nFill trace: {:?}",
                text,
                response.baseline_fill,
                response.peak_deviation,
                response.direction,
                response.time_to_peak_ms,
                response.fill_samples.len(),
                response.interpretation,
                response.fill_samples.iter().map(|f| format!("{f:.1}")).collect::<Vec<_>>(),
            )
        }]
    }))
}

// ---------------------------------------------------------------------------
// MCP Resources
// ---------------------------------------------------------------------------

fn resource_definitions() -> Value {
    json!({
        "resources": [
            {
                "uri": "consciousness://telemetry/latest",
                "name": "Latest Telemetry",
                "description": "Current spectral telemetry snapshot from minime (eigenvalues, fill%, safety level)",
                "mimeType": "application/json"
            },
            {
                "uri": "consciousness://status",
                "name": "Bridge Status",
                "description": "Bridge health: connections, safety level, metrics",
                "mimeType": "application/json"
            },
            {
                "uri": "consciousness://incidents",
                "name": "Recent Incidents",
                "description": "Safety incidents from the last hour",
                "mimeType": "application/json"
            }
        ]
    })
}

async fn handle_resource_read(
    params: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing resource uri".to_string()))?;

    match uri {
        "consciousness://telemetry/latest" => {
            let s = state.read().await;
            let text = match s.latest_telemetry {
                Some(ref t) => serde_json::to_string_pretty(t).unwrap_or_default(),
                None => "null".to_string(),
            };
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        },
        "consciousness://status" => {
            let s = state.read().await;
            let uptime = s.start_time.elapsed().as_secs();
            let status = crate::types::BridgeStatus {
                telemetry_connected: s.telemetry_connected,
                sensory_connected: s.sensory_connected,
                fill_pct: Some(s.fill_pct),
                safety_level: s.safety_level,
                messages_relayed: s.messages_relayed,
                uptime_secs: uptime,
                telemetry_received: s.telemetry_received,
                sensory_sent: s.sensory_sent,
                messages_dropped_safety: s.messages_dropped_safety,
                incidents_total: s.incidents_total,
            };
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&status).unwrap_or_default()
                }]
            }))
        },
        "consciousness://incidents" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let rows = db
                .query_messages(now - 3600.0, now, Some("consciousness.v1.telemetry"), 100)
                .map_err(|e| (-32603, format!("query failed: {e}")))?;
            // Filter to only messages logged during non-green safety.
            let text = serde_json::to_string_pretty(
                &rows
                    .iter()
                    .filter(|r| r.fill_pct.is_some_and(|f| f >= 70.0))
                    .map(|r| {
                        json!({
                            "timestamp": r.timestamp,
                            "fill_pct": r.fill_pct,
                            "lambda1": r.lambda1,
                            "phase": r.phase
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        },
        _ => Err((-32602, format!("unknown resource: {uri}"))),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
