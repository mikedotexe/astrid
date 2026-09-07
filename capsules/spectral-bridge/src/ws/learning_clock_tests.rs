use super::*;

async fn packet(state: &Arc<RwLock<BridgeState>>, db: &Arc<BridgeDb>, t_ms: u64) {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "t_ms": t_ms, "eigenvalues": [768.0, 300.0], "fill_ratio": 0.71,
    }))
    .unwrap();
    assert!(handle_telemetry_message_at(&bytes, state, db, 100.0).await);
}

#[tokio::test]
async fn learning_clock_is_bound_to_decoded_packet_and_current_connection() {
    let state = Arc::new(RwLock::new(BridgeState::new()));
    let db = Arc::new(BridgeDb::open(":memory:").unwrap());
    {
        let mut shared = state.write().await;
        record_connected(&mut shared, WsLane::Telemetry, 1, 99.0);
    }
    packet(&state, &db, 1000).await;
    let original = state.read().await.learning_observation().unwrap();
    assert_eq!(original.producer_t_ms, 1000);
    assert!(!handle_telemetry_message_at(b"invalid", &state, &db, 101.0).await);
    assert_eq!(state.read().await.learning_observation(), Some(original));
    {
        let mut shared = state.write().await;
        record_disconnected(&mut shared, WsLane::Telemetry, "test boundary".into());
        assert!(shared.learning_observation().is_none());
        record_connected(&mut shared, WsLane::Telemetry, 2, 102.0);
        assert!(shared.learning_observation().is_none());
    }
    // Reconnect can leave producer uptime increasing. It is still an observation gap.
    packet(&state, &db, 1001).await;
    let reconnected = state.read().await.learning_observation().unwrap();
    assert_ne!(original.scope, reconnected.scope);
    packet(&state, &db, 10).await;
    assert!(state.read().await.learning_observation().is_none());
    packet(&state, &db, 11).await;
    let regressed = state.read().await.learning_observation().unwrap();
    assert_ne!(reconnected.scope, regressed.scope);
    packet(&state, &db, 11).await;
    assert!(state.read().await.learning_observation().is_none());
    assert_eq!(
        state.read().await.latest_telemetry.as_ref().unwrap().t_ms,
        11
    );
}
