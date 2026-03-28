use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::sync::RwLock;

use super::*;

fn unique_temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("bridge_{name}_{stamp}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_test_wav(path: &PathBuf) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample_idx in 0..16_000_u32 {
        let t = (sample_idx as f32) / 16_000.0_f32;
        let sample = (2.0_f32 * std::f32::consts::PI * 220.0_f32 * t).sin() * 0.3_f32;
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
}

#[test]
fn tool_definitions_has_all_tools() {
    let defs = tool_definitions();
    let tools = defs["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"get_latest_telemetry"));
    assert!(names.contains(&"get_bridge_status"));
    assert!(names.contains(&"send_control"));
    assert!(names.contains(&"send_semantic"));
    assert!(names.contains(&"query_message_log"));
    assert!(names.contains(&"send_text"));
    assert!(names.contains(&"send_text_and_observe"));
    assert!(names.contains(&"interpret_consciousness"));
    assert!(names.contains(&"render_chimera"));
}

#[test]
fn initialize_response_has_required_fields() {
    let resp = handle_initialize().unwrap();
    assert!(resp.get("protocolVersion").is_some());
    assert!(resp.get("capabilities").is_some());
    assert!(resp.get("serverInfo").is_some());
}

#[test]
fn json_rpc_response_success_format() {
    let resp = JsonRpcResponse::success(json!(1), json!({"ok": true}));
    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[test]
fn json_rpc_response_error_format() {
    let resp = JsonRpcResponse::error(json!(1), -32600, "bad request");
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "bad request");
}

#[test]
fn resource_definitions_has_all_resources() {
    let defs = resource_definitions();
    let resources = defs["resources"].as_array().unwrap();
    let uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert!(uris.contains(&"consciousness://telemetry/latest"));
    assert!(uris.contains(&"consciousness://status"));
    assert!(uris.contains(&"consciousness://incidents"));
}

#[test]
fn initialize_advertises_resources() {
    let resp = handle_initialize().unwrap();
    assert!(resp["capabilities"]["resources"].is_object());
}

#[tokio::test]
async fn resource_read_telemetry_when_empty() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let params = json!({"uri": "consciousness://telemetry/latest"});
    let result = handle_resource_read(&params, &state, &db).await.unwrap();
    let text = result["contents"][0]["text"].as_str().unwrap();
    assert_eq!(text, "null");
}

#[tokio::test]
async fn resource_read_status() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let params = json!({"uri": "consciousness://status"});
    let result = handle_resource_read(&params, &state, &db).await.unwrap();
    let text = result["contents"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["safety_level"], "green");
    assert_eq!(parsed["telemetry_connected"], false);
}

#[tokio::test]
async fn resource_read_unknown_uri() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let params = json!({"uri": "consciousness://nonexistent"});
    let result = handle_resource_read(&params, &state, &db).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn render_chimera_tool_returns_structured_content() {
    let temp_dir = unique_temp_dir("mcp_render");
    let input_path = temp_dir.join("input.wav");
    let output_dir = temp_dir.join("render_output");
    write_test_wav(&input_path);

    let response = tool_render_chimera(&json!({
        "input_path": input_path,
        "output_root": output_dir,
        "mode": "dual",
        "loops": 1
    }))
    .await
    .unwrap();

    assert!(response["structuredContent"]["output_dir"].is_string());
    assert_eq!(response["structuredContent"]["mode"], "dual");
    assert!(response["structuredContent"]["manifest_path"].is_string());
    assert!(response["structuredContent"]["iterations"].is_array());

    let _ = fs::remove_dir_all(&temp_dir);
}
