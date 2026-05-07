use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use super::*;
use crate::types::SpectralTelemetry;

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
    assert!(names.contains(&"record_attractor_intent"));
    assert!(names.contains(&"record_attractor_observation"));
    assert!(names.contains(&"query_attractor_ledger"));
    assert!(names.contains(&"send_text"));
    assert!(names.contains(&"send_text_and_observe"));
    assert!(names.contains(&"interpret_consciousness"));
    assert!(names.contains(&"probe_action"));
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

#[test]
fn probe_action_normalizes_trailing_next_line() {
    let parsed = normalize_probe_action("Astrid response\nNEXT: LIST_FILES /tmp").unwrap();
    assert_eq!(parsed, "LIST_FILES /tmp");
}

#[test]
fn probe_action_uses_self_observation_query_fallback() {
    let db = crate::db::BridgeDb::open(":memory:").unwrap();
    db.save_self_observation(
        crate::db::unix_now(),
        1,
        "resonance topology geometry landscape",
        "excerpt",
    )
    .unwrap();

    let query = probe_effective_search_query("SEARCH", &db).unwrap();
    assert!(query.contains("resonance"));
}

#[tokio::test]
async fn probe_action_list_files_returns_context_and_logs() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let dir = unique_temp_dir("probe_ls");
    fs::write(dir.join("note.txt"), "hello").unwrap();

    let result = tool_probe_action(
        &json!({"action_text": format!("Prelude\nNEXT: LIST_FILES {}", dir.display())}),
        &state,
        &db,
    )
    .await
    .unwrap();

    assert_eq!(result["structuredContent"]["status"], "ok");
    let experienced = result["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(experienced.contains("[Directory listing you requested:]"));
    assert!(experienced.contains("note.txt"));

    let rows = db
        .query_messages(0.0, f64::MAX, Some(PROBE_TOPIC), 10)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].direction, "operator_probe");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn probe_action_browse_and_read_more_use_probe_state() {
    clear_probe_read_more_state();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let page_body = "Manipulable relationships between eigenvalue branches and perception remain relevant to the current question. ".repeat(64);
    let server_body = page_body.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 1024];
        let _ = stream.read(&mut buf).await.unwrap();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
            server_body.len(),
            server_body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });

    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let browse = tool_probe_action(
        &json!({"action_text": format!("BROWSE http://{addr}/page")}),
        &state,
        &db,
    )
    .await
    .unwrap();
    server.await.unwrap();

    assert_eq!(browse["structuredContent"]["status"], "ok");
    let browse_text = browse["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(browse_text.contains("Relevant knowledge from the web:"));
    assert!(browse_text.contains("Why it may matter:"));
    assert!(browse_text.contains("[You read the page at"));
    assert!(probe_state_path().exists());

    let artifact_path = browse["structuredContent"]["artifacts"][0]["path"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(PathBuf::from(&artifact_path).exists());

    let read_more = tool_probe_action(&json!({"action_text": "READ_MORE"}), &state, &db)
        .await
        .unwrap();
    assert_eq!(read_more["structuredContent"]["status"], "ok");
    let read_more_text = read_more["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(read_more_text.contains("[Meaning summary from this document:]"));
    assert!(read_more_text.contains("[Continuing reading from offset"));

    let _ = fs::remove_file(artifact_path);
    clear_probe_read_more_state();
}

#[tokio::test]
async fn probe_action_browse_soft_failure_returns_explicit_failure() {
    clear_probe_read_more_state();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 1024];
        let _ = stream.read(&mut buf).await.unwrap();
        let body = "<html><title>Page Not Found</title><body>Page Not Found. The page you are trying to reach cannot be found. Error.</body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });

    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let browse = tool_probe_action(
        &json!({"action_text": format!("BROWSE http://{addr}/missing")}),
        &state,
        &db,
    )
    .await
    .unwrap();
    server.await.unwrap();

    assert_eq!(browse["structuredContent"]["status"], "error");
    let experienced = browse["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(experienced.contains("could not be meaningfully read"));
    assert!(experienced.contains("NEXT: SEARCH"));

    let state_file = load_probe_read_more_state().unwrap_or_default();
    assert!(state_file.last_read_path.is_none());
    assert!(state_file.last_read_meaning_summary.is_none());
    clear_probe_read_more_state();
}

#[tokio::test]
async fn probe_action_missing_input_returns_structured_error() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());

    let result = tool_probe_action(&json!({}), &state, &db).await.unwrap();
    assert_eq!(result["structuredContent"]["status"], "error");
    assert_eq!(result["isError"], true);
}

#[tokio::test]
async fn probe_action_unsupported_returns_structured_status() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());

    let result = tool_probe_action(&json!({"action_text": "PING"}), &state, &db)
        .await
        .unwrap();
    assert_eq!(result["structuredContent"]["status"], "unsupported");
}

#[tokio::test]
async fn attractor_ledger_tools_roundtrip() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());

    let intent_response = tool_record_attractor_intent(
        &json!({
            "author": "astrid",
            "substrate": "triple_reservoir",
            "label": "garden fold",
            "command": "create",
            "goal": "same-prompt reconvergence after quiet",
            "intervention_plan": {
                "mode": "garden_clone",
                "vector_schedule": [[0.1, -0.1, 0.0]],
                "rehearsal_mode": "hold"
            },
            "safety_bounds": {
                "max_fill_pct": 88.0,
                "allow_live_control": false,
                "rollback_on_red": true
            }
        }),
        &db,
    )
    .unwrap();
    let intent_id = intent_response["structuredContent"]["intent_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(intent_id.starts_with("attr-"));

    let observation_response = tool_record_attractor_observation(
        &json!({
            "intent_id": intent_id,
            "substrate": "triple_reservoir",
            "label": "garden fold",
            "recurrence_score": 0.72,
            "authorship_score": 0.64,
            "safety_level": "green",
            "basin_shift_score": 0.14
        }),
        &state,
        &db,
    )
    .await
    .unwrap();
    assert_eq!(
        observation_response["structuredContent"]["classification"],
        "authored"
    );

    let ledger = tool_query_attractor_ledger(&json!({"limit": 10}), &db).unwrap();
    let rows = ledger["structuredContent"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["record_type"], "observation");
    assert_eq!(rows[1]["record_type"], "intent");

    let intent_messages = db
        .query_messages(0.0, f64::MAX, Some(ATTRACTOR_INTENT_TOPIC), 10)
        .unwrap();
    let observation_messages = db
        .query_messages(0.0, f64::MAX, Some(ATTRACTOR_OBSERVATION_TOPIC), 10)
        .unwrap();
    assert_eq!(intent_messages.len(), 1);
    assert_eq!(observation_messages.len(), 1);
}

#[tokio::test]
async fn bold_send_control_requires_attractor_intent_and_logs_command() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());
    let (tx, mut rx) = mpsc::channel(4);

    let blocked = tool_send_control(
        &json!({
            "target_lambda_bias": 0.05,
            "pi_kp": 0.1
        }),
        &state,
        &db,
        &tx,
    )
    .await
    .unwrap();
    assert_eq!(blocked["isError"], true);
    assert!(rx.try_recv().is_err());

    let sent = tool_send_control(
        &json!({
            "target_lambda_bias": 0.05,
            "pi_kp": 0.1,
            "pi_ki": 0.01,
            "pi_max_step": 0.02,
            "attractor_intent_id": "attr-scoped-1"
        }),
        &state,
        &db,
        &tx,
    )
    .await
    .unwrap();
    assert!(sent.get("isError").is_none());

    match rx.try_recv().unwrap() {
        SensoryMsg::Control {
            target_lambda_bias,
            pi_kp,
            pi_ki,
            pi_max_step,
            ..
        } => {
            assert_eq!(target_lambda_bias, Some(0.05));
            assert_eq!(pi_kp, Some(0.1));
            assert_eq!(pi_ki, Some(0.01));
            assert_eq!(pi_max_step, Some(0.02));
        },
        _ => panic!("wrong variant"),
    }

    let rows = db
        .query_messages(0.0, f64::MAX, Some(ATTRACTOR_COMMAND_TOPIC), 10)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].payload.contains("attr-scoped-1"));
    assert!(rows[0].payload.contains("target_lambda_bias"));
}

#[tokio::test]
async fn probe_action_compose_returns_experienced_text_and_artifact() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    {
        let mut state = state.write().await;
        state.latest_telemetry = Some(SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![828.5, 312.1, 45.7],
            fill_ratio: 0.552,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: Some(vec![0.0; 32]),
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            structural_entropy: None,
            resonance_density_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
        });
        state.spectral_fingerprint = Some(vec![0.0; 32]);
    }
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());

    let result = tool_probe_action(&json!({"action_text": "COMPOSE"}), &state, &db)
        .await
        .unwrap();

    assert_eq!(result["structuredContent"]["status"], "ok");
    let experienced = result["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(experienced.contains("You composed audio from your spectral state:"));

    let artifact_path = result["structuredContent"]["artifacts"][0]["path"]
        .as_str()
        .unwrap();
    assert!(PathBuf::from(artifact_path).exists());
    let _ = fs::remove_file(artifact_path);
}

#[tokio::test]
async fn probe_action_autoresearch_list_returns_context_when_repo_exists() {
    if !crate::paths::bridge_paths().autoresearch_root().exists() {
        return;
    }

    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(crate::db::BridgeDb::open(":memory:").unwrap());

    let result = tool_probe_action(&json!({"action_text": "AR_LIST"}), &state, &db)
        .await
        .unwrap();

    assert_eq!(result["structuredContent"]["status"], "ok");
    let experienced = result["structuredContent"]["experienced_text"]
        .as_str()
        .unwrap();
    assert!(experienced.contains("[Autoresearch]"));

    let artifact_path = result["structuredContent"]["artifacts"][0]["path"]
        .as_str()
        .unwrap();
    assert!(PathBuf::from(artifact_path).exists());
}
