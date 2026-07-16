/// Channel for sending sensory messages to Minime.
pub type SensorySender = mpsc::Sender<SensoryMsg>;

/// Spawn the sensory `WebSocket` sender task.
///
/// Connects to minime's sensory input on port 7879 and forwards
/// `SensoryMsg` values received from the channel. Respects safety
/// protocol — suspends sending when fill is orange/red.
#[expect(clippy::too_many_lines)]
pub fn spawn_sensory_sender(
    url: String,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    mut rx: mpsc::Receiver<SensoryMsg>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new();
        let mut shutdown = shutdown;

        loop {
            if *shutdown.borrow() {
                info!("sensory sender shutting down");
                return;
            }

            let connection_id = {
                let mut s = state.write().await;
                record_connect_attempt(&mut s, WsLane::Sensory)
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
                        let mut s = state.write().await;
                        record_connected(&mut s, WsLane::Sensory, connection_id, unix_now_s());
                    }

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();

                    let disconnect_reason = loop {
                        tokio::select! {
                            _ = shutdown.changed() => {
                                info!("sensory sender received shutdown");
                                let _ = ws_tx.close().await;
                                return;
                            }
                            // Forward outbound messages to minime.
                            msg = rx.recv() => {
                                if let Some(sensory_msg) = msg {
                                    // Check safety before sending.
                                    let safety = state.read().await.safety_level;
                                    if safety.should_suspend_outbound() {
                                        warn!(
                                            safety = ?safety,
                                            "dropping outbound message — safety protocol"
                                        );
                                        {
                                            let mut s = state.write().await;
                                            s.messages_dropped_safety = s.messages_dropped_safety.saturating_add(1);
                                        }
                                        continue;
                                    }
                                    // Semantic packets are policy-shaped before queueing.
                                    // Re-running the plain rescue block here discards already
                                    // budgeted limited-write packets after status records them.

                                    let json = match encode_sensory_packet(&sensory_msg) {
                                        Ok(j) => j,
                                        Err(e) => {
                                            error!(error = %e, "failed to serialize sensory msg");
                                            continue;
                                        }
                                    };

                                    // Log before sending.
                                    let (fill_pct, lambda1) = {
                                        let s = state.read().await;
                                        (s.fill_pct, s.latest_telemetry.as_ref().map(SpectralTelemetry::lambda1))
                                    };
                                    let _ = db.log_message(
                                        MessageDirection::AstridToMinime,
                                        "consciousness.v1.sensory",
                                        &json,
                                        Some(fill_pct),
                                        lambda1,
                                        None,
                                    );

                                    let json_len = json.len();
                                    if let Err(e) = ws_tx.send(Message::Text(json.clone())).await {
                                        let reason = format!("send_error:{e}");
                                        {
                                            let mut s = state.write().await;
                                            record_ws_send_error(
                                                &mut s,
                                                WsLane::Sensory,
                                                reason.clone(),
                                            );
                                        }
                                        error!(error = %e, "failed to send to minime");
                                        break reason;
                                    }
                                    trace_ws_send(
                                        WsLane::Sensory,
                                        connection_id,
                                        "text",
                                        Some(json_len),
                                    );
                                    if let Err(e) = trace_lab::record_sensory_send(
                                        &sensory_msg,
                                        &json,
                                        fill_pct,
                                        lambda1,
                                        unix_now_s(),
                                    ) {
                                        warn!(error = %e, "failed to record trace lab sensory send event");
                                    }

                                    {
                                        let mut s = state.write().await;
                                        s.messages_relayed = s.messages_relayed.saturating_add(1);
                                        s.sensory_sent = s.sensory_sent.saturating_add(1);
                                        record_ws_message_sent(&mut s, WsLane::Sensory);
                                    }
                                } else {
                                    info!("sensory channel closed");
                                    return;
                                }
                            }
                            // Handle incoming messages (pings, closes).
                            ws_msg = ws_rx.next() => {
                                    match ws_msg {
                                        Some(Ok(Message::Ping(data))) => {
                                            trace_ws_receive(
                                                WsLane::Sensory,
                                                connection_id,
                                                "ping",
                                                Some(data.len()),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Sensory,
                                                    "ping",
                                                );
                                            }
                                            let bytes = data.len();
                                            if let Err(e) = ws_tx.send(Message::Pong(data)).await {
                                                let reason = format!("pong_send_error:{e}");
                                                {
                                                    let mut s = state.write().await;
                                                    record_ws_send_error(
                                                        &mut s,
                                                        WsLane::Sensory,
                                                        reason.clone(),
                                                    );
                                                }
                                                break reason;
                                            }
                                            trace_ws_send(
                                                WsLane::Sensory,
                                                connection_id,
                                                "pong",
                                                Some(bytes),
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_sent(&mut s, WsLane::Sensory);
                                            }
                                        }
                                        Some(Ok(Message::Pong(_))) => {
                                            trace_ws_receive(
                                                WsLane::Sensory,
                                                connection_id,
                                                "pong",
                                                None,
                                            );
                                            {
                                                let mut s = state.write().await;
                                                record_ws_message_received(
                                                    &mut s,
                                                    WsLane::Sensory,
                                                    "pong",
                                                );
                                            }
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            let reason = close_reason(frame);
                                            warn!(
                                                reason = %reason,
                                                "sensory WebSocket closed"
                                            );
                                            break reason;
                                        }
                                        None => {
                                            warn!("sensory WebSocket stream ended");
                                            break String::from("stream_ended");
                                        }
                                        Some(Err(e)) => {
                                            let reason = format!("websocket_error:{e}");
                                            error!(error = %e, "sensory WebSocket error");
                                            break reason;
                                        }
                                    _ => {}
                                }
                            }
                        }
                    };

                    {
                        let mut s = state.write().await;
                        record_disconnected(&mut s, WsLane::Sensory, disconnect_reason.clone());
                    }
                    connection_span.in_scope(|| {
                        warn!(
                            reason = %disconnect_reason,
                            duration_secs = connection_started.elapsed().as_secs_f64(),
                            "sensory WebSocket connection ended"
                        );
                    });
                },
                Err(e) => {
                    {
                        let mut s = state.write().await;
                        record_connect_error(&mut s, WsLane::Sensory, format!("connect_error:{e}"));
                    }
                    warn!(
                        error = %e,
                        lane = WsLane::Sensory.as_str(),
                        connection_id,
                        "failed to connect to minime sensory input"
                    );
                },
            }

            let delay = backoff.next_delay();
            {
                let mut s = state.write().await;
                record_reconnect_scheduled(&mut s, WsLane::Sensory);
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
