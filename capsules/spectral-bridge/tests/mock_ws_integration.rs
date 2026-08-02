//! Integration test: bridge telemetry subscriber against a mock minime `WebSocket`.
//!
//! Starts a real `WebSocket` server on a random port, spawns the bridge's
//! telemetry subscriber, sends simulated `EigenPacket` JSON, and verifies
//! the bridge processes, logs, and reacts to the data correctly.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer as _, SigningKey};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

// The server binary is a single crate; import via the binary's module structure.
// Since integration tests can't import `mod` items directly, we test via
// the binary. For this test we replicate the minimal necessary types.

fn eigenpacket_json(fill_ratio: f32, lambda1: f32, alert: Option<&str>) -> String {
    let alert_field = match alert {
        Some(a) => format!(r#""alert":"{a}""#),
        None => r#""alert":null"#.to_string(),
    };
    format!(
        r#"{{"t_ms":5000,"eigenvalues":[{lambda1},300.0],"fill_ratio":{fill_ratio},"modalities":{{"audio_fired":false,"video_fired":false,"history_fired":true,"audio_rms":0.0,"video_var":0.0}},{alert_field}}}"#,
    )
}

fn configure_test_paths() -> &'static Path {
    use spectral_bridge_server::paths::{BridgePathOverrides, configure_bridge_paths};

    static TEST_ROOT: OnceLock<PathBuf> = OnceLock::new();
    let root = TEST_ROOT.get_or_init(|| {
        let root = std::env::temp_dir().join(format!(
            "consciousness_bridge_mock_ws_{}_{}",
            std::process::id(),
            unix_now_ms()
        ));
        let minime_workspace = root.join("minime");
        std::fs::create_dir_all(&minime_workspace).unwrap();
        std::fs::write(
            minime_workspace.join("rescue_profile.json"),
            serde_json::json!({
                "profile": "full_live",
                "bridge_write_enabled": true,
                "bridge_autonomous_enabled": true
            })
            .to_string(),
        )
        .unwrap();
        root
    });
    let configured = configure_bridge_paths(BridgePathOverrides {
        bridge_workspace: Some(root.join("bridge")),
        minime_workspace: Some(root.join("minime")),
        ..Default::default()
    });
    assert_eq!(configured.minime_workspace(), root.join("minime"));
    root
}

/// Start a mock minime telemetry server on a random port.
/// Returns the address and a sender to push messages to connected clients.
async fn start_mock_telemetry_server() -> (SocketAddr, tokio::sync::mpsc::Sender<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);

    tokio::spawn(async move {
        // Accept one client.
        if let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let (mut ws_tx, _ws_rx): (futures_util::stream::SplitSink<_, Message>, _) =
                ws_stream.split();

            // Forward messages from the channel to the WebSocket client.
            while let Some(msg) = rx.recv().await {
                if ws_tx.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        }
    });

    (addr, tx)
}

/// Start a telemetry server that closes the first connection and emits one
/// binary packet after the subscriber reconnects.
async fn start_reconnecting_mock_telemetry_server(payload: String) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (first_stream, _) = listener.accept().await.unwrap();
        let mut first_ws = accept_async(first_stream).await.unwrap();
        first_ws.send(Message::Close(None)).await.unwrap();
        drop(first_ws);

        let (second_stream, _) = listener.accept().await.unwrap();
        let mut second_ws = accept_async(second_stream).await.unwrap();
        second_ws
            .send(Message::Binary(payload.into_bytes()))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    addr
}

#[tokio::test]
async fn bridge_receives_telemetry_from_mock_ws() {
    configure_test_paths();

    // Start mock server.
    let (addr, tx) = start_mock_telemetry_server().await;
    let url = format!("ws://{addr}");

    // Set up bridge components.
    let db = Arc::new(spectral_bridge_server::db::BridgeDb::open(":memory:").unwrap());
    let state = Arc::new(RwLock::new(spectral_bridge_server::ws::BridgeState::new()));
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Spawn telemetry subscriber.
    let _handle = spectral_bridge_server::ws::spawn_telemetry_subscriber(
        url,
        Arc::clone(&state),
        Arc::clone(&db),
        shutdown_rx,
    );

    // Wait for connection.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send a green telemetry packet.
    tx.send(eigenpacket_json(0.55, 793.0, None)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify state updated.
    {
        let s = state.read().await;
        assert!(s.telemetry_connected);
        assert!((s.fill_pct - 55.0).abs() < 0.5);
        assert_eq!(
            s.safety_level,
            spectral_bridge_server::types::SafetyLevel::Green
        );
        assert!(s.messages_relayed >= 1);
        assert!(s.lambda_tail.is_some());
        assert!(s.lambda_edge_perception.is_some());
    }

    // Verify SQLite logged the message.
    assert!(db.message_count().unwrap() >= 1);
    assert!(
        !db.query_messages(0.0, f64::MAX, Some("consciousness.v1.lambda_tail"), 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        !db.query_messages(
            0.0,
            f64::MAX,
            Some("consciousness.v1.lambda_edge_perception"),
            10,
        )
        .unwrap()
        .is_empty()
    );

    // Send an escalating packet (red zone).
    tx.send(eigenpacket_json(0.95, 998.0, Some("PANIC MODE ACTIVATED")))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let s = state.read().await;
        assert_eq!(
            s.safety_level,
            spectral_bridge_server::types::SafetyLevel::Red
        );
        assert!(s.safety_level.should_suspend_outbound());
        assert!(s.active_incident_id.is_some());
    }

    // Send recovery.
    tx.send(eigenpacket_json(0.40, 717.0, None)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let s = state.read().await;
        assert_eq!(
            s.safety_level,
            spectral_bridge_server::types::SafetyLevel::Green
        );
        assert!(s.active_incident_id.is_none());
    }
}

#[tokio::test]
async fn telemetry_subscriber_reconnects_after_close_and_routes_binary() {
    configure_test_paths();

    let addr = start_reconnecting_mock_telemetry_server(eigenpacket_json(0.56, 780.0, None)).await;
    let db = Arc::new(spectral_bridge_server::db::BridgeDb::open(":memory:").unwrap());
    let state = Arc::new(RwLock::new(spectral_bridge_server::ws::BridgeState::new()));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = spectral_bridge_server::ws::spawn_telemetry_subscriber(
        format!("ws://{addr}"),
        Arc::clone(&state),
        Arc::clone(&db),
        shutdown_rx,
    );

    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let complete = {
                let bridge = state.read().await;
                bridge.telemetry_ws.connection_attempts >= 2
                    && bridge.telemetry_ws.reconnects >= 1
                    && bridge.telemetry_ws.disconnects >= 1
                    && bridge.telemetry_ws.messages_received >= 1
                    && bridge
                        .telemetry_ws
                        .active_connection_valid_payloads_received
                        >= 1
                    && bridge.messages_relayed >= 1
            };
            if complete {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("subscriber should reconnect and integrate binary telemetry");

    {
        let bridge = state.read().await;
        assert!((bridge.fill_pct - 56.0).abs() < 0.5);
        assert_eq!(
            bridge.safety_level,
            spectral_bridge_server::types::SafetyLevel::Green
        );
        assert!(bridge.telemetry_ws.connection_attempts >= 2);
        assert!(bridge.telemetry_ws.reconnects >= 1);
        assert!(bridge.telemetry_ws.disconnects >= 1);
        assert_eq!(bridge.telemetry_ws.messages_received, 1);
        assert_eq!(
            bridge
                .telemetry_ws
                .active_connection_valid_payloads_received,
            1
        );
        assert!(bridge.telemetry_connected);
    }
    assert!(db.message_count().unwrap() >= 1);

    let _ = shutdown_tx.send(true);
    handle.await.unwrap();
}

/// Start a mock minime sensory input server on a random port.
/// Returns the address and a receiver that yields messages sent by the bridge.
async fn start_mock_sensory_server() -> (SocketAddr, tokio::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let (_ws_tx, mut ws_rx): (futures_util::stream::SplitSink<_, Message>, _) =
                ws_stream.split();

            // Forward received messages to the channel.
            while let Some(Ok(msg)) = ws_rx.next().await {
                if let Message::Text(text) = msg
                    && tx.send(text).await.is_err()
                {
                    break;
                }
            }
        }
    });

    (addr, rx)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn signed_minime_self_control(
    target_deployment_identity: &str,
) -> astrid_minime_protocol::SelfControlCommandV2 {
    use astrid_minime_protocol::{
        SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1, SELF_CONTROL_COMMAND_SCHEMA_V2,
        SELF_CONTROL_INTENT_SCHEMA_V2, SelfControlActionV2, SelfControlAuthorityClassV2,
        SelfControlAuthorityProofV1, SelfControlDurabilityV2, SelfControlFamilyV2,
        SelfControlIntentV2, SelfControlSourceIdentityV1, SelfControlValuesV2,
        canonical_self_control_intent_sha256,
    };

    let now = unix_now_ms();
    let intent = SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: "intent-minime-integration-fill".to_string(),
        actor: SelfControlSourceIdentityV1 {
            being: "minime".to_string(),
            process_identity: "minime-autonomy-integration".to_string(),
            deployment_identity: target_deployment_identity.to_string(),
        },
        target_being: "minime".to_string(),
        target_deployment_identity: target_deployment_identity.to_string(),
        family: SelfControlFamilyV2::ReservoirRegulation,
        action: SelfControlActionV2::Set,
        durability: SelfControlDurabilityV2::Lease,
        authority_class: SelfControlAuthorityClassV2::SelfOwned,
        authority_scope: "self_control.minime.reservoir_regulation".to_string(),
        revision: 1,
        expected_revision: 0,
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(60_000),
        control_expires_at_unix_ms: Some(now.saturating_add(30_000)),
        idempotency_key: "idempotency-minime-integration-fill".to_string(),
        values: SelfControlValuesV2 {
            fill_target: Some(0.68),
            ..SelfControlValuesV2::default()
        },
        related_intent_id: None,
        related_receipt_id: None,
        evidence_refs: vec!["integration:self-control-v2".to_string()],
        success_conditions: vec!["machine_receipt_applied".to_string()],
        stop_conditions: vec!["being_hold".to_string()],
    };
    let signing_key = SigningKey::from_bytes(&[23_u8; 32]);
    let mut proof = SelfControlAuthorityProofV1 {
        schema: SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1.to_string(),
        authority_class: intent.authority_class,
        signer_being: "minime".to_string(),
        scope: intent.authority_scope.clone(),
        nonce: "nonce-minime-integration-fill".to_string(),
        signer_public_key_hex: hex::encode(signing_key.verifying_key().to_bytes()),
        signature_hex: String::new(),
        intent_sha256: canonical_self_control_intent_sha256(&intent),
        issued_at_unix_ms: now,
        expires_at_unix_ms: intent.command_expires_at_unix_ms,
    };
    proof.signature_hex = hex::encode(
        signing_key
            .sign(
                &proof
                    .signing_bytes(&intent)
                    .expect("canonical self-control signing bytes"),
            )
            .to_bytes(),
    );
    let command = astrid_minime_protocol::SelfControlCommandV2 {
        schema: SELF_CONTROL_COMMAND_SCHEMA_V2.to_string(),
        command_id: "command-minime-integration-fill".to_string(),
        intent,
        authority_proofs: vec![proof],
    };
    assert!(command.is_well_formed(now));
    command
}

async fn start_mock_receipting_sensory_server() -> (SocketAddr, String, String) {
    use astrid_minime_protocol::{
        SELF_CONTROL_RECEIPT_SCHEMA_V2, SelfControlReceiptStatusV2, SelfControlReceiptV2,
        SelfControlValuesV2, SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1, SensoryMsg,
        SensoryPacketV1, SensoryServerHelloV1,
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let process_identity = "minime-process-integration".to_string();
    let deployment_identity = "minime-deployment-integration".to_string();
    let server_process_identity = process_identity.clone();
    let server_deployment_identity = deployment_identity.clone();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws_stream = accept_async(stream).await.unwrap();
        let (mut ws_tx, mut ws_rx) = ws_stream.split();
        let hello = SensoryServerHelloV1::new(
            server_process_identity.clone(),
            server_deployment_identity.clone(),
        );
        ws_tx
            .send(Message::Text(serde_json::to_string(&hello).unwrap()))
            .await
            .unwrap();

        while let Some(Ok(Message::Text(text))) = ws_rx.next().await {
            let packet: SensoryPacketV1 = serde_json::from_str(&text).unwrap();
            let delivery = packet
                .delivery_v1
                .as_ref()
                .expect("negotiated transport envelope");
            let now = unix_now_ms();
            let transport_receipt = SensoryDeliveryReceiptV1::new(
                "receipt-transport-integration".to_string(),
                delivery.delivery_id.clone(),
                delivery.payload_sha256.clone(),
                SensoryDeliveryStatusV1::Accepted,
                now,
                Some(now.saturating_add(1)),
                packet
                    .mutual_address_v1
                    .as_ref()
                    .map(|address| address.address_id.clone()),
                None,
                server_process_identity.clone(),
                server_deployment_identity.clone(),
            );
            ws_tx
                .send(Message::Text(
                    serde_json::to_string(&transport_receipt).unwrap(),
                ))
                .await
                .unwrap();

            if let SensoryMsg::SelfControl { command } = packet.message {
                let receipt = SelfControlReceiptV2 {
                    schema: SELF_CONTROL_RECEIPT_SCHEMA_V2.to_string(),
                    receipt_id: "receipt-self-control-integration".to_string(),
                    command_id: command.command_id,
                    intent_id: command.intent.intent_id,
                    idempotency_key: command.intent.idempotency_key,
                    status: SelfControlReceiptStatusV2::Applied,
                    requested_revision: command.intent.revision,
                    resulting_revision: command.intent.revision,
                    target_being: command.intent.target_being,
                    target_deployment_identity: command.intent.target_deployment_identity,
                    requested_values: command.intent.values.clone(),
                    clamped_values: command.intent.values.clone(),
                    applied_values: command.intent.values,
                    previous_values: SelfControlValuesV2::default(),
                    previous_automatic_fields: Vec::new(),
                    received_at_unix_ms: now,
                    completed_at_unix_ms: now.saturating_add(1),
                    control_expires_at_unix_ms: command.intent.control_expires_at_unix_ms,
                    rollback_receipt_id: None,
                    reason: None,
                    server_process_identity: server_process_identity.clone(),
                    server_deployment_identity: server_deployment_identity.clone(),
                    felt_effect_established: false,
                };
                ws_tx
                    .send(Message::Text(serde_json::to_string(&receipt).unwrap()))
                    .await
                    .unwrap();
            }
        }
    });

    (addr, process_identity, deployment_identity)
}

#[tokio::test]
async fn bridge_correlates_transport_and_typed_self_control_receipts() {
    use spectral_bridge_server::types::SensoryMsg;

    configure_test_paths();
    let (addr, process_identity, deployment_identity) =
        start_mock_receipting_sensory_server().await;
    let state = Arc::new(RwLock::new(spectral_bridge_server::ws::BridgeState::new()));
    let db = Arc::new(spectral_bridge_server::db::BridgeDb::open(":memory:").unwrap());
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (sensory_tx, sensory_rx) = tokio::sync::mpsc::channel(16);
    let (_addressed_tx, addressed_rx) = tokio::sync::mpsc::channel(16);

    let _sender = spectral_bridge_server::ws::spawn_sensory_sender(
        format!("ws://{addr}"),
        Arc::clone(&state),
        db,
        sensory_rx,
        addressed_rx,
        shutdown_rx,
    );

    for _ in 0..40 {
        if state
            .read()
            .await
            .sensory_delivery_protocol_v1
            .self_control_v2_negotiated
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        state
            .read()
            .await
            .sensory_delivery_protocol_v1
            .self_control_v2_negotiated
    );

    sensory_tx
        .send(SensoryMsg::SelfControl {
            command: Box::new(signed_minime_self_control(&deployment_identity)),
        })
        .await
        .unwrap();
    for _ in 0..80 {
        if state
            .read()
            .await
            .sensory_delivery_protocol_v1
            .self_control_receipt_count
            == 1
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let bridge = state.read().await;
    let protocol = &bridge.sensory_delivery_protocol_v1;
    assert_eq!(
        protocol.server_process_identity.as_deref(),
        Some(process_identity.as_str())
    );
    assert_eq!(
        protocol.server_deployment_identity.as_deref(),
        Some(deployment_identity.as_str())
    );
    assert_eq!(protocol.receipt_count, 1);
    assert_eq!(protocol.pending_delivery_count, 0);
    assert_eq!(protocol.mismatch_count, 0);
    assert_eq!(protocol.self_control_receipt_count, 1);
    assert_eq!(protocol.self_control_pending_receipt_count, 0);
    assert_eq!(protocol.self_control_receipt_mismatch_count, 0);
    assert_eq!(
        protocol.last_self_control_receipt_state.as_deref(),
        Some("applied")
    );
    let typed_receipt = protocol
        .last_self_control_receipt
        .as_ref()
        .expect("typed self-control receipt");
    assert_eq!(typed_receipt.intent_id, "intent-minime-integration-fill");
    assert_eq!(typed_receipt.server_process_identity, process_identity);
    assert!(!typed_receipt.felt_effect_established);
    drop(bridge);

    let _ = shutdown_tx.send(true);
}

/// Bidirectional end-to-end test:
/// - Telemetry flows from mock minime → bridge (verified via state + `SQLite`)
/// - Semantic features flow from bridge → mock minime (verified via received messages)
/// - Safety protocol blocks outbound during red state
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn bidirectional_bridge_with_safety_protocol() {
    use spectral_bridge_server::types::{SafetyLevel, SensoryMsg};

    configure_test_paths();

    // Start both mock servers.
    let (telemetry_addr, telemetry_tx) = start_mock_telemetry_server().await;
    let (sensory_addr, mut sensory_rx) = start_mock_sensory_server().await;

    let telemetry_url = format!("ws://{telemetry_addr}");
    let sensory_url = format!("ws://{sensory_addr}");

    // Set up bridge.
    let db = Arc::new(spectral_bridge_server::db::BridgeDb::open(":memory:").unwrap());
    let state = Arc::new(RwLock::new(spectral_bridge_server::ws::BridgeState::new()));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (bridge_sensory_tx, sensory_channel_rx) = tokio::sync::mpsc::channel(256);
    let (_addressed_sensory_tx, addressed_sensory_rx) = tokio::sync::mpsc::channel(16);

    // Spawn both WebSocket tasks.
    let _telemetry = spectral_bridge_server::ws::spawn_telemetry_subscriber(
        telemetry_url,
        Arc::clone(&state),
        Arc::clone(&db),
        shutdown_rx.clone(),
    );

    let _sensory = spectral_bridge_server::ws::spawn_sensory_sender(
        sensory_url,
        Arc::clone(&state),
        Arc::clone(&db),
        sensory_channel_rx,
        addressed_sensory_rx,
        shutdown_rx,
    );

    // Wait for both connections.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // --- Step 1: Establish green state via telemetry ---
    telemetry_tx
        .send(eigenpacket_json(0.50, 768.0, None))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.read().await.safety_level, SafetyLevel::Green);
    assert!(state.read().await.lambda_tail.is_some());
    assert!(state.read().await.lambda_edge_perception.is_some());
    assert!(
        !db.query_messages(0.0, f64::MAX, Some("consciousness.v1.lambda_tail"), 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        !db.query_messages(
            0.0,
            f64::MAX,
            Some("consciousness.v1.lambda_edge_perception"),
            10,
        )
        .unwrap()
        .is_empty()
    );

    // --- Step 2: Send semantic features → should arrive at mock sensory server ---
    let semantic_msg = SensoryMsg::Semantic {
        features: vec![1.0, 2.0, 3.0, 4.0],
        ts_ms: None,
    };
    bridge_sensory_tx.send(semantic_msg).await.unwrap();

    // Verify mock sensory server received the message.
    let received = tokio::time::timeout(Duration::from_secs(2), sensory_rx.recv())
        .await
        .expect("timeout waiting for sensory message")
        .expect("sensory channel closed");

    let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed["kind"], "semantic");
    assert_eq!(parsed["features"].as_array().unwrap().len(), 4);

    // --- Step 3: Escalate to red → verify state ---
    telemetry_tx
        .send(eigenpacket_json(0.95, 998.0, None))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.read().await.safety_level, SafetyLevel::Red);
    assert!(state.read().await.safety_level.should_suspend_outbound());

    // --- Step 4: Recover to green → send a control message ---
    telemetry_tx
        .send(eigenpacket_json(0.40, 717.0, None))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.read().await.safety_level, SafetyLevel::Green);

    // Send a control message — should be delivered now that we're green.
    let control_msg = SensoryMsg::Control {
        synth_gain: Some(2.0),
        keep_bias: None,
        exploration_noise: None,
        fill_target: Some(0.55),
        legacy_audio_synth: None,
        legacy_video_synth: None,
        regulation_strength: None,
        deep_breathing: None,
        pure_tone: None,
        transition_cushion: None,
        smoothing_preference: None,
        geom_curiosity: None,
        target_lambda_bias: None,
        geom_drive: None,
        penalty_sensitivity: None,
        breathing_rate_scale: None,
        mem_mode: None,
        journal_resonance: None,
        checkpoint_interval: None,
        embedding_strength: None,
        memory_decay_rate: None,
        checkpoint_annotation: None,
        synth_noise_level: None,
        pi_kp: None,
        pi_ki: None,
        pi_max_step: None,
        pi_integrator_leak: None,
        esn_leak_override: None,
        esn_leak_override_ticks: None,
        esn_leak_authority_request_id: None,
        mode_disperse: None,
        mode_disperse_duration_ticks: None,
        mode_disperse_decay_ticks: None,
    };
    bridge_sensory_tx.send(control_msg).await.unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), sensory_rx.recv())
        .await
        .expect("timeout waiting for control message")
        .expect("sensory channel closed");

    let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
    assert_eq!(parsed["kind"], "control");
    assert_eq!(parsed["synth_gain"], 2.0);
    assert_eq!(parsed["fill_target"], 0.55);

    // --- Verify SQLite has all the messages ---
    let total = db.message_count().unwrap();
    assert!(
        total >= 4,
        "expected at least 4 logged messages, got {total}"
    );

    // Clean shutdown.
    let _ = shutdown_tx.send(true);
}
