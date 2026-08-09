use std::{
    collections::{HashSet, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use astrid_minime_protocol::{
    SensoryDeliveryReceiptV1, SensoryDeliveryStatusV1, SensoryMsg, SensoryPacketV1,
    SensoryServerHelloV1,
};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::{
    net::TcpListener,
    sync::{Mutex, broadcast, mpsc, watch},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::reservoir::{ReservoirSnapshot, SensoryIngress};

const DEDUP_CAPACITY: usize = 4_096;

struct Dedup {
    order: VecDeque<String>,
    ids: HashSet<String>,
}

impl Dedup {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            ids: HashSet::new(),
        }
    }

    fn observe(&mut self, id: &str) -> bool {
        if self.ids.contains(id) {
            return true;
        }
        let owned = id.to_string();
        self.ids.insert(owned.clone());
        self.order.push_back(owned);
        while self.order.len() > DEDUP_CAPACITY {
            if let Some(expired) = self.order.pop_front() {
                self.ids.remove(&expired);
            }
        }
        false
    }
}

pub async fn serve_telemetry(
    address: SocketAddr,
    telemetry_tx: broadcast::Sender<String>,
    snapshots: watch::Receiver<ReservoirSnapshot>,
) {
    let listener = TcpListener::bind(address)
        .await
        .unwrap_or_else(|error| panic!("bind telemetry {address}: {error}"));
    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            continue;
        };
        let mut receiver = telemetry_tx.subscribe();
        let latest = snapshots.borrow().telemetry_json.clone();
        tokio::spawn(async move {
            let Ok(mut socket) = accept_async(stream).await else {
                return;
            };
            if !latest.is_empty() && socket.send(Message::Text(latest)).await.is_err() {
                return;
            }
            loop {
                tokio::select! {
                    message = receiver.recv() => {
                        let Ok(message) = message else { return };
                        if socket.send(Message::Text(message)).await.is_err() {
                            return;
                        }
                    },
                    incoming = socket.next() => {
                        match incoming {
                            Some(Ok(Message::Ping(data))) => {
                                if socket.send(Message::Pong(data)).await.is_err() {
                                    return;
                                }
                            },
                            Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                            _ => {},
                        }
                    },
                }
            }
        });
    }
}

#[allow(clippy::too_many_lines)] // The connection task keeps receipt and routing order explicit.
pub async fn serve_sensory(address: SocketAddr, ingress_tx: mpsc::Sender<SensoryIngress>) {
    let listener = TcpListener::bind(address)
        .await
        .unwrap_or_else(|error| panic!("bind sensory {address}: {error}"));
    let dedup = Arc::new(Mutex::new(Dedup::new()));
    let process_identity = format!(
        "pid:{}:started_at_unix_ms:{}",
        std::process::id(),
        unix_millis()
    );
    let deployment_identity = format!(
        "astrid-edge-runtime:{}:{}",
        env!("CARGO_PKG_VERSION"),
        option_env!("ASTRID_EDGE_SOURCE_COMMIT").unwrap_or("unknown")
    );

    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            continue;
        };
        let ingress_tx = ingress_tx.clone();
        let dedup = Arc::clone(&dedup);
        let process_identity = process_identity.clone();
        let deployment_identity = deployment_identity.clone();
        tokio::spawn(async move {
            let Ok(mut socket) = accept_async(stream).await else {
                return;
            };
            let hello = cpu_edge_hello(SensoryServerHelloV1::new(
                process_identity.clone(),
                deployment_identity.clone(),
            ));
            if socket
                .send(Message::Text(
                    serde_json::to_string(&hello).unwrap_or_default(),
                ))
                .await
                .is_err()
            {
                return;
            }

            while let Some(message) = socket.next().await {
                let Ok(Message::Text(text)) = message else {
                    continue;
                };
                let packet = match serde_json::from_str::<SensoryPacketV1>(&text) {
                    Ok(packet) => packet,
                    Err(error) => {
                        eprintln!("rejected malformed sensory packet: {error}");
                        continue;
                    },
                };
                let received_at = unix_millis();
                let compatibility = packet.compatibility();
                let mut status = if !compatibility.is_compatible() {
                    SensoryDeliveryStatusV1::Rejected
                } else if external_control_kind(&packet.message).is_some() {
                    SensoryDeliveryStatusV1::PolicyBlocked
                } else {
                    dimensional_status(&packet.message)
                };
                let mut reason = if compatibility.is_compatible() {
                    external_control_kind(&packet.message).map(ToOwned::to_owned)
                } else {
                    Some(format!("unsupported_protocol:{compatibility:?}"))
                };

                if let Some(delivery) = &packet.delivery_v1 {
                    if !delivery.payload_matches(&packet.message) {
                        status = SensoryDeliveryStatusV1::Rejected;
                        reason = Some("payload_sha256_mismatch".to_string());
                    } else if dedup.lock().await.observe(&delivery.delivery_id) {
                        status = SensoryDeliveryStatusV1::Duplicate;
                        reason = Some("deduplication_window".to_string());
                    }
                }

                if matches!(
                    status,
                    SensoryDeliveryStatusV1::Accepted | SensoryDeliveryStatusV1::PartiallyApplied
                ) {
                    if let Some(ingress) = message_to_ingress(packet.message.clone()) {
                        if ingress_tx.send(ingress).await.is_err() {
                            return;
                        }
                    } else {
                        status = SensoryDeliveryStatusV1::PolicyBlocked;
                        reason = Some("unsupported_on_cpu_edge".to_string());
                    }
                }

                if let Some(delivery) = packet.delivery_v1 {
                    let receipt = SensoryDeliveryReceiptV1::new(
                        format!("receipt:{}:{received_at}", delivery.delivery_id),
                        delivery.delivery_id,
                        delivery.payload_sha256,
                        status,
                        received_at,
                        matches!(
                            status,
                            SensoryDeliveryStatusV1::Accepted
                                | SensoryDeliveryStatusV1::PartiallyApplied
                        )
                        .then_some(unix_millis()),
                        packet.mutual_address_v1.map(|address| address.address_id),
                        reason,
                        process_identity.clone(),
                        deployment_identity.clone(),
                    );
                    if socket
                        .send(Message::Text(
                            serde_json::to_string(&receipt).unwrap_or_default(),
                        ))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        });
    }
}

fn cpu_edge_hello(mut hello: SensoryServerHelloV1) -> SensoryServerHelloV1 {
    hello.capabilities.retain(|capability| {
        !capability.starts_with("division_") && !capability.starts_with("self_control_")
    });
    hello
        .capabilities
        .push("external_reservoir_control_policy_blocked".to_string());
    hello
        .capabilities
        .push("reservoir_tuning_private_action_executor_only".to_string());
    hello
}

fn dimensional_status(message: &SensoryMsg) -> SensoryDeliveryStatusV1 {
    let exact = match message {
        SensoryMsg::Video { features, .. } | SensoryMsg::Audio { features, .. } => {
            features.len() == 8
        },
        SensoryMsg::Aux { features, .. } => features.len() == 2,
        SensoryMsg::Semantic { features, .. } => features.len() == 48,
        _ => false,
    };
    if exact {
        SensoryDeliveryStatusV1::Accepted
    } else {
        SensoryDeliveryStatusV1::PartiallyApplied
    }
}

fn message_to_ingress(message: SensoryMsg) -> Option<SensoryIngress> {
    match message {
        SensoryMsg::Video { features, .. } => Some(SensoryIngress::Video {
            features,
            source: "external_websocket_video".to_string(),
        }),
        SensoryMsg::Audio { features, .. } => Some(SensoryIngress::Audio {
            features,
            source: "external_websocket_audio".to_string(),
        }),
        SensoryMsg::Aux { features, .. } => Some(SensoryIngress::Aux {
            features,
            source: "external_sensory_bus".to_string(),
            availability: None,
        }),
        SensoryMsg::Semantic { features, .. } => Some(SensoryIngress::Semantic(features)),
        _ => None,
    }
}

fn external_control_kind(message: &SensoryMsg) -> Option<&'static str> {
    match message {
        SensoryMsg::Control { .. } => {
            Some("legacy_control_policy_blocked_private_action_executor_only")
        },
        _ => None,
    }
}

fn unix_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{cpu_edge_hello, external_control_kind, message_to_ingress};
    use astrid_minime_protocol::{SensoryMsg, SensoryServerHelloV1};

    #[test]
    fn legacy_control_has_no_external_ingress_authority() {
        let message = serde_json::from_value::<SensoryMsg>(serde_json::json!({
            "kind": "control",
            "fill_target": 0.75,
            "synth_gain": 1.2
        }))
        .unwrap();
        assert!(external_control_kind(&message).is_some());
        assert!(message_to_ingress(message).is_none());
    }

    #[test]
    fn hello_truthfully_withholds_external_self_control_capabilities() {
        let hello = cpu_edge_hello(SensoryServerHelloV1::new(
            "process".to_string(),
            "deployment".to_string(),
        ));
        assert!(
            hello
                .capabilities
                .iter()
                .all(|value| !value.starts_with("self_control_"))
        );
        assert!(
            hello
                .capabilities
                .iter()
                .any(|value| value == "external_reservoir_control_policy_blocked")
        );
        assert!(
            hello
                .capabilities
                .iter()
                .any(|value| value == "reservoir_tuning_private_action_executor_only")
        );
    }
}
