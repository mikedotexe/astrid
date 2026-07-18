/// Channel for sending sensory messages to Minime.
pub type SensorySender = mpsc::Sender<SensoryMsg>;

type SensoryWsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;

#[allow(clippy::too_many_arguments)]
async fn send_sensory_message_v1(
    ws_tx: &mut SensoryWsSink,
    sensory_msg: SensoryMsg,
    mutual_address_v1: Option<astrid_minime_protocol::MutualAddressEnvelopeV1>,
    receipts_negotiated: bool,
    sequence: u64,
    connection_id: u64,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Option<PendingSensoryDeliveryV1>, String> {
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        warn!(
            safety = ?safety,
            "dropping outbound message — safety protocol"
        );
        let mut bridge = state.write().await;
        bridge.messages_dropped_safety = bridge.messages_dropped_safety.saturating_add(1);
        return Ok(None);
    }

    let encoded = encode_sensory_packet_v1(
        &sensory_msg,
        mutual_address_v1,
        receipts_negotiated,
        sequence,
    )
    .map_err(|error| format!("serialization_error:{error}"))?;

    let (fill_pct, lambda1) = {
        let bridge = state.read().await;
        (
            bridge.fill_pct,
            bridge
                .latest_telemetry
                .as_ref()
                .map(SpectralTelemetry::lambda1),
        )
    };
    let _ = db.log_message(
        MessageDirection::AstridToMinime,
        "consciousness.v1.sensory",
        &encoded.json,
        Some(fill_pct),
        lambda1,
        None,
    );

    let json_len = encoded.json.len();
    if let Err(error) = ws_tx.send(Message::Text(encoded.json.clone())).await {
        let reason = format!("send_error:{error}");
        let mut bridge = state.write().await;
        record_ws_send_error(&mut bridge, WsLane::Sensory, reason.clone());
        return Err(reason);
    }
    trace_ws_send(
        WsLane::Sensory,
        connection_id,
        "text",
        Some(json_len),
    );
    if let Err(error) = trace_lab::record_sensory_send(
        &sensory_msg,
        &encoded.json,
        fill_pct,
        lambda1,
        unix_now_s(),
    ) {
        warn!(error = %error, "failed to record trace lab sensory send event");
    }

    {
        let mut bridge = state.write().await;
        bridge.messages_relayed = bridge.messages_relayed.saturating_add(1);
        bridge.sensory_sent = bridge.sensory_sent.saturating_add(1);
        record_ws_message_sent(&mut bridge, WsLane::Sensory);
        if encoded.pending.is_some() {
            bridge
                .sensory_delivery_protocol_v1
                .sent_with_delivery_count = bridge
                .sensory_delivery_protocol_v1
                .sent_with_delivery_count
                .saturating_add(1);
        }
    }
    if let Some(pending) = encoded.pending.as_ref() {
        record_pending_delivery(pending);
    }
    Ok(encoded.pending)
}

fn record_pending_unknown_v1(
    state: &mut BridgeState,
    pending: &mut BTreeMap<String, PendingSensoryDeliveryV1>,
    reason: &str,
) {
    record_unknown_deliveries(
        pending,
        reason,
        &mut state.sensory_delivery_protocol_v1,
    );
}

/// Spawn the sensory `WebSocket` sender task.
///
/// Legacy and protocol 1.0 peers receive the exact existing packet shape.
/// After a valid 1.1 hello, every packet carries technical delivery identity.
/// Missing receipts become evidence-only `unknown_delivery` rows and are never
/// automatically resent.
#[expect(clippy::too_many_lines)]
pub fn spawn_sensory_sender(
    url: String,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    mut rx: mpsc::Receiver<SensoryMsg>,
    mut addressed_rx: mpsc::Receiver<AddressedSensoryMessage>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new();
        let mut shutdown = shutdown;
        let mut sequence = 0_u64;
        let mut ordinary_open = true;
        let mut addressed_open = true;

        loop {
            if *shutdown.borrow() {
                info!("sensory sender shutting down");
                return;
            }

            let connection_id = {
                let mut bridge = state.write().await;
                record_connect_attempt(&mut bridge, WsLane::Sensory)
            };
            info!(
                url = %url,
                lane = WsLane::Sensory.as_str(),
                connection_id,
                "connecting to minime sensory input"
            );

            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    let connection_started = Instant::now();
                    let connection_span = info_span!(
                        "ws.connection",
                        lane = WsLane::Sensory.as_str(),
                        connection_id,
                        url = %url
                    );
                    connection_span.in_scope(|| info!("connected to minime sensory input"));
                    backoff.reset();

                    {
                        let mut bridge = state.write().await;
                        record_connected(
                            &mut bridge,
                            WsLane::Sensory,
                            connection_id,
                            unix_now_s(),
                        );
                        bridge.sensory_delivery_protocol_v1.negotiated = false;
                        bridge.sensory_delivery_protocol_v1.server_process_identity = None;
                        bridge.sensory_delivery_protocol_v1.server_deployment_identity = None;
                    }

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();
                    let mut receipts_negotiated = false;
                    let mut pending = BTreeMap::<String, PendingSensoryDeliveryV1>::new();

                    let disconnect_reason = loop {
                        tokio::select! {
                            _ = shutdown.changed() => {
                                info!("sensory sender received shutdown");
                                {
                                    let mut bridge = state.write().await;
                                    record_pending_unknown_v1(
                                        &mut bridge,
                                        &mut pending,
                                        "bridge_shutdown",
                                    );
                                }
                                let _ = ws_tx.close().await;
                                return;
                            }
                            msg = rx.recv(), if ordinary_open => {
                                let Some(sensory_msg) = msg else {
                                    ordinary_open = false;
                                    if !addressed_open {
                                        let mut bridge = state.write().await;
                                        record_pending_unknown_v1(
                                            &mut bridge,
                                            &mut pending,
                                            "sensory_channels_closed",
                                        );
                                        return;
                                    }
                                    continue;
                                };
                                sequence = sequence.saturating_add(1);
                                match send_sensory_message_v1(
                                    &mut ws_tx,
                                    sensory_msg,
                                    None,
                                    receipts_negotiated,
                                    sequence,
                                    connection_id,
                                    &state,
                                    &db,
                                ).await {
                                    Ok(Some(item)) => {
                                        pending.insert(item.delivery_id.clone(), item);
                                        let mut bridge = state.write().await;
                                        bridge.sensory_delivery_protocol_v1.pending_delivery_count =
                                            pending.len().try_into().unwrap_or(u64::MAX);
                                    },
                                    Ok(None) => {},
                                    Err(reason) => break reason,
                                }
                            }
                            msg = addressed_rx.recv(), if addressed_open => {
                                let Some(addressed) = msg else {
                                    addressed_open = false;
                                    if !ordinary_open {
                                        let mut bridge = state.write().await;
                                        record_pending_unknown_v1(
                                            &mut bridge,
                                            &mut pending,
                                            "sensory_channels_closed",
                                        );
                                        return;
                                    }
                                    continue;
                                };
                                let (sensory_msg, mutual_address_v1) = addressed.into_parts();
                                sequence = sequence.saturating_add(1);
                                match send_sensory_message_v1(
                                    &mut ws_tx,
                                    sensory_msg,
                                    Some(mutual_address_v1),
                                    receipts_negotiated,
                                    sequence,
                                    connection_id,
                                    &state,
                                    &db,
                                ).await {
                                    Ok(Some(item)) => {
                                        pending.insert(item.delivery_id.clone(), item);
                                        let mut bridge = state.write().await;
                                        bridge.sensory_delivery_protocol_v1.pending_delivery_count =
                                            pending.len().try_into().unwrap_or(u64::MAX);
                                    },
                                    Ok(None) => {},
                                    Err(reason) => break reason,
                                }
                            }
                            ws_msg = ws_rx.next() => {
                                match ws_msg {
                                    Some(Ok(Message::Text(text))) => {
                                        trace_ws_receive(
                                            WsLane::Sensory,
                                            connection_id,
                                            "text",
                                            Some(text.len()),
                                        );
                                        let mut bridge = state.write().await;
                                        record_ws_message_received(
                                            &mut bridge,
                                            WsLane::Sensory,
                                            "text",
                                        );
                                        if let Ok(hello) =
                                            serde_json::from_str::<
                                                astrid_minime_protocol::SensoryServerHelloV1,
                                            >(&text)
                                        {
                                            receipts_negotiated = apply_server_hello(
                                                hello,
                                                &mut bridge.sensory_delivery_protocol_v1,
                                            );
                                        } else if let Ok(receipt) =
                                            serde_json::from_str::<
                                                astrid_minime_protocol::SensoryDeliveryReceiptV1,
                                            >(&text)
                                        {
                                            let _ = apply_delivery_receipt(
                                                receipt,
                                                &mut pending,
                                                &mut bridge.sensory_delivery_protocol_v1,
                                            );
                                        }
                                    },
                                    Some(Ok(Message::Ping(data))) => {
                                        trace_ws_receive(
                                            WsLane::Sensory,
                                            connection_id,
                                            "ping",
                                            Some(data.len()),
                                        );
                                        {
                                            let mut bridge = state.write().await;
                                            record_ws_message_received(
                                                &mut bridge,
                                                WsLane::Sensory,
                                                "ping",
                                            );
                                        }
                                        let bytes = data.len();
                                        if let Err(error) = ws_tx.send(Message::Pong(data)).await {
                                            let reason = format!("pong_send_error:{error}");
                                            let mut bridge = state.write().await;
                                            record_ws_send_error(
                                                &mut bridge,
                                                WsLane::Sensory,
                                                reason.clone(),
                                            );
                                            break reason;
                                        }
                                        trace_ws_send(
                                            WsLane::Sensory,
                                            connection_id,
                                            "pong",
                                            Some(bytes),
                                        );
                                        let mut bridge = state.write().await;
                                        record_ws_message_sent(&mut bridge, WsLane::Sensory);
                                    },
                                    Some(Ok(Message::Pong(_))) => {
                                        trace_ws_receive(
                                            WsLane::Sensory,
                                            connection_id,
                                            "pong",
                                            None,
                                        );
                                        let mut bridge = state.write().await;
                                        record_ws_message_received(
                                            &mut bridge,
                                            WsLane::Sensory,
                                            "pong",
                                        );
                                    },
                                    Some(Ok(Message::Close(frame))) => {
                                        let reason = close_reason(frame);
                                        warn!(reason = %reason, "sensory WebSocket closed");
                                        break reason;
                                    },
                                    None => {
                                        warn!("sensory WebSocket stream ended");
                                        break String::from("stream_ended");
                                    },
                                    Some(Err(error)) => {
                                        let reason = format!("websocket_error:{error}");
                                        error!(error = %error, "sensory WebSocket error");
                                        break reason;
                                    },
                                    _ => {},
                                }
                            }
                        }
                    };

                    {
                        let mut bridge = state.write().await;
                        record_pending_unknown_v1(
                            &mut bridge,
                            &mut pending,
                            &disconnect_reason,
                        );
                        record_disconnected(
                            &mut bridge,
                            WsLane::Sensory,
                            disconnect_reason.clone(),
                        );
                    }
                    connection_span.in_scope(|| {
                        warn!(
                            reason = %disconnect_reason,
                            duration_secs = connection_started.elapsed().as_secs_f64(),
                            "sensory WebSocket connection ended"
                        );
                    });
                },
                Err(error) => {
                    {
                        let mut bridge = state.write().await;
                        record_connect_error(
                            &mut bridge,
                            WsLane::Sensory,
                            format!("connect_error:{error}"),
                        );
                    }
                    warn!(
                        error = %error,
                        lane = WsLane::Sensory.as_str(),
                        connection_id,
                        "failed to connect to minime sensory input"
                    );
                },
            }

            let delay = backoff.next_delay();
            {
                let mut bridge = state.write().await;
                record_reconnect_scheduled(&mut bridge, WsLane::Sensory);
            }
            info!(
                delay_secs = delay.as_secs(),
                lane = WsLane::Sensory.as_str(),
                connection_id,
                "reconnecting to sensory"
            );

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("sensory sender shutting down during backoff");
                    return;
                }
                () = tokio::time::sleep(delay) => {}
            }
        }
    })
}
