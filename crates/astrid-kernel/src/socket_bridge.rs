//! Native Unix socket bridge for CLI clients.
//!
//! This keeps daemon management reachable even when legacy uplink capsules fail
//! to load. It speaks the length-prefixed JSON management protocol:
//! authenticated handshake first, then `IpcMessage` frames.

use std::sync::Arc;

use astrid_core::session_token::{
    HandshakeRequest, HandshakeResponse, PROTOCOL_VERSION, SessionToken,
};
use astrid_events::ipc::{IpcMessage, IpcPayload, IpcTraceContextV1};
use astrid_events::{AstridEvent, EventMetadata};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tracing::{debug, error, warn};

const MAX_HANDSHAKE_SIZE: usize = 4096;
const MAX_IPC_FRAME_SIZE: usize = 50 * 1024 * 1024;
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const BRIDGE_SOURCE: &str = "native_socket_bridge";
const SENSORY_USER_INPUT_TOPIC: &str = "sensory.v1.user_input";

/// Spawn the native CLI socket bridge.
#[must_use]
pub(crate) fn spawn_native_socket_bridge(
    kernel: Arc<crate::Kernel>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Some(listener) = kernel.cli_socket_listener.clone() else {
            warn!("native socket bridge not started; no bound listener");
            return;
        };

        loop {
            let accepted = listener.lock().await.accept().await;
            match accepted {
                Ok((stream, _addr)) => {
                    let kernel = Arc::clone(&kernel);
                    tokio::spawn(async move {
                        handle_connection(kernel, stream).await;
                    });
                },
                Err(e) => {
                    error!(error = %e, "native socket bridge accept failed");
                    break;
                },
            }
        }
    })
}

async fn handle_connection(kernel: Arc<crate::Kernel>, mut stream: UnixStream) {
    if authenticate(&kernel, &mut stream).await.is_err() {
        return;
    }

    let connection_source = format!("{BRIDGE_SOURCE}:{}", uuid::Uuid::new_v4());
    publish_client_event(&kernel, &connection_source, IpcPayload::Connect);
    let (read_half, write_half) = stream.into_split();
    let writer = tokio::spawn(forward_events_to_client(
        Arc::clone(&kernel),
        write_half,
        connection_source.clone(),
    ));
    read_client_messages(Arc::clone(&kernel), read_half, &connection_source).await;
    writer.abort();
    publish_client_event(
        &kernel,
        &connection_source,
        IpcPayload::Disconnect { reason: None },
    );
}

async fn authenticate(
    kernel: &Arc<crate::Kernel>,
    stream: &mut UnixStream,
) -> Result<(), std::io::Error> {
    let request = match tokio::time::timeout(HANDSHAKE_TIMEOUT, read_json_frame(stream)).await {
        Ok(Ok(req)) => req,
        Ok(Err(e)) => {
            let _ = write_json_frame(stream, &HandshakeResponse::error(e.to_string())).await;
            return Err(e);
        },
        Err(_) => {
            let _ =
                write_json_frame(stream, &HandshakeResponse::error("handshake timed out")).await;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "handshake timed out",
            ));
        },
    };

    let response = validate_handshake(kernel, &request);
    write_json_frame(stream, &response).await?;
    if response.is_ok() {
        debug!(client_version = %request.client_version, "native socket handshake accepted");
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            response
                .reason
                .unwrap_or_else(|| "handshake rejected".to_string()),
        ))
    }
}

fn validate_handshake(
    kernel: &Arc<crate::Kernel>,
    request: &HandshakeRequest,
) -> HandshakeResponse {
    if request.protocol_version != PROTOCOL_VERSION {
        return HandshakeResponse::error(format!(
            "unsupported protocol version {}; expected {}",
            request.protocol_version, PROTOCOL_VERSION
        ));
    }

    match SessionToken::from_hex(&request.token) {
        Ok(token) if token.ct_eq(&kernel.session_token) => HandshakeResponse::ok(),
        Ok(_) => HandshakeResponse::error("invalid session token"),
        Err(e) => HandshakeResponse::error(format!("invalid session token: {e}")),
    }
}

async fn read_client_messages(
    kernel: Arc<crate::Kernel>,
    mut read_half: OwnedReadHalf,
    connection_source: &str,
) {
    loop {
        match read_ipc_frame(&mut read_half).await {
            Ok(Some(mut message)) => {
                ensure_user_input_trace(&mut message);
                if let Some(sensory_message) = sensory_user_input_mirror(&message) {
                    let _ = kernel.event_bus.publish(AstridEvent::Ipc {
                        metadata: traced_event_metadata(connection_source, &sensory_message),
                        message: sensory_message,
                    });
                }
                let _ = kernel.event_bus.publish(AstridEvent::Ipc {
                    metadata: traced_event_metadata(connection_source, &message),
                    message,
                });
            },
            Ok(None) => break,
            Err(e) => {
                warn!(error = %e, "native socket bridge read failed");
                break;
            },
        }
    }
}

fn traced_event_metadata(source: &str, message: &IpcMessage) -> EventMetadata {
    let mut metadata = EventMetadata::new(source);
    if let Some(trace) = message.trace.as_ref().filter(|trace| trace.is_supported()) {
        metadata = metadata.with_correlation_id(trace.trace_id);
    }
    metadata
}

fn ensure_user_input_trace(message: &mut IpcMessage) {
    let IpcPayload::UserInput { session_id, .. } = &message.payload else {
        return;
    };
    let trace_id = message
        .trace
        .as_ref()
        .filter(|trace| trace.is_supported())
        .map_or_else(uuid::Uuid::new_v4, |trace| trace.trace_id);
    let chain_id = message
        .trace
        .as_ref()
        .filter(|trace| trace.is_supported())
        .and_then(|trace| trace.chain_id.clone());
    message.trace = Some(IpcTraceContextV1::root(
        trace_id,
        session_id.clone(),
        chain_id,
    ));
}

fn sensory_user_input_mirror(message: &IpcMessage) -> Option<IpcMessage> {
    if !matches!(message.payload, IpcPayload::UserInput { .. }) {
        return None;
    }
    let mut mirrored = message.clone();
    mirrored.topic = SENSORY_USER_INPUT_TOPIC.to_string();
    Some(mirrored)
}

async fn forward_events_to_client(
    kernel: Arc<crate::Kernel>,
    mut write_half: OwnedWriteHalf,
    connection_source: String,
) {
    let mut receiver = kernel.event_bus.subscribe();
    while let Some(event) = receiver.recv().await {
        let AstridEvent::Ipc { metadata, message } = &*event else {
            continue;
        };
        if !should_forward_event(&metadata.source, &connection_source, &message.topic) {
            continue;
        }
        if let Err(e) = write_json_frame(&mut write_half, message).await {
            debug!(error = %e, "native socket bridge write failed");
            break;
        }
    }
}

fn should_forward_event(event_source: &str, connection_source: &str, topic: &str) -> bool {
    event_source != connection_source && !topic.starts_with("client.v1.")
}

fn publish_client_event(kernel: &Arc<crate::Kernel>, connection_source: &str, payload: IpcPayload) {
    let topic = match &payload {
        IpcPayload::Connect => "client.v1.connect",
        IpcPayload::Disconnect { .. } => "client.v1.disconnect",
        _ => return,
    };
    let message = IpcMessage::new(topic, payload, kernel.session_id.0);
    let _ = kernel.event_bus.publish(AstridEvent::Ipc {
        metadata: EventMetadata::new(connection_source),
        message,
    });
}

async fn read_ipc_frame(
    read_half: &mut OwnedReadHalf,
) -> Result<Option<IpcMessage>, std::io::Error> {
    let mut len_buf = [0_u8; 4];
    if let Err(e) = read_half.read_exact(&mut len_buf).await {
        return if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Ok(None)
        } else {
            Err(e)
        };
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_IPC_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("IPC frame too large: {len} bytes"),
        ));
    }
    let mut payload = vec![0_u8; len];
    read_half.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload).map(Some).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid IPC frame JSON: {e}"),
        )
    })
}

async fn read_json_frame<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
) -> Result<T, std::io::Error> {
    let mut len_buf = [0_u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_HANDSHAKE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("handshake frame too large: {len} bytes"),
        ));
    }
    let mut payload = vec![0_u8; len];
    stream.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid handshake JSON: {e}"),
        )
    })
}

async fn write_json_frame<T: serde::Serialize, W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    value: &T,
) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "frame exceeds 4 GiB"))?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::{
        SENSORY_USER_INPUT_TOPIC, ensure_user_input_trace, sensory_user_input_mirror,
        should_forward_event, traced_event_metadata,
    };
    use astrid_events::ipc::{IpcMessage, IpcPayload, IpcTraceContextV1};
    use uuid::Uuid;

    #[test]
    fn socket_client_does_not_receive_its_own_events() {
        assert!(!should_forward_event(
            "native_socket_bridge:one",
            "native_socket_bridge:one",
            "user.v1.prompt",
        ));
    }

    #[test]
    fn authenticated_peer_can_observe_another_clients_events() {
        assert!(should_forward_event(
            "native_socket_bridge:one",
            "native_socket_bridge:observer",
            "user.v1.prompt",
        ));
    }

    #[test]
    fn connection_lifecycle_remains_internal() {
        assert!(!should_forward_event(
            "native_socket_bridge:one",
            "native_socket_bridge:observer",
            "client.v1.connect",
        ));
    }

    #[test]
    fn user_input_gets_an_explicit_passive_sensory_mirror() {
        let message = IpcMessage::new(
            "user.v1.prompt",
            IpcPayload::UserInput {
                text: "hello".to_string(),
                session_id: "session".to_string(),
                context: None,
            },
            Uuid::nil(),
        );
        let mirrored = sensory_user_input_mirror(&message).expect("user input should be mirrored");
        assert_eq!(mirrored.topic, SENSORY_USER_INPUT_TOPIC);
        assert_eq!(mirrored.payload, message.payload);

        let response = IpcMessage::new(
            "agent.v1.response",
            IpcPayload::AgentResponse {
                text: "hello".to_string(),
                is_final: true,
                session_id: "session".to_string(),
            },
            Uuid::nil(),
        );
        assert!(sensory_user_input_mirror(&response).is_none());
    }

    #[test]
    fn user_input_gets_root_trace_and_sensory_mirror_preserves_it() {
        let supplied_trace_id = Uuid::new_v4();
        let mut message = IpcMessage::new(
            "user.v1.prompt",
            IpcPayload::UserInput {
                text: "hello".to_string(),
                session_id: "session-one".to_string(),
                context: None,
            },
            Uuid::nil(),
        )
        .with_trace(IpcTraceContextV1::root(
            supplied_trace_id,
            "untrusted-old-session",
            Some("chain-one".to_string()),
        ));

        ensure_user_input_trace(&mut message);
        let trace = message.trace.as_ref().unwrap();
        assert_eq!(trace.trace_id, supplied_trace_id);
        assert_eq!(trace.session_id.as_deref(), Some("session-one"));
        assert_eq!(trace.chain_id.as_deref(), Some("chain-one"));
        assert!(trace.parent_span_id.is_none());

        let mirrored = sensory_user_input_mirror(&message).unwrap();
        assert_eq!(mirrored.trace, message.trace);
        assert_eq!(
            traced_event_metadata("test", &mirrored).correlation_id,
            Some(supplied_trace_id),
        );
    }

    #[test]
    fn malformed_trace_is_replaced_without_affecting_user_payload() {
        let mut message = IpcMessage::new(
            "user.v1.prompt",
            IpcPayload::UserInput {
                text: "hello".to_string(),
                session_id: "session".to_string(),
                context: None,
            },
            Uuid::nil(),
        );
        message.trace = Some(IpcTraceContextV1 {
            schema_version: 99,
            trace_id: Uuid::nil(),
            span_id: Uuid::nil(),
            parent_span_id: None,
            session_id: None,
            chain_id: None,
        });
        let payload = message.payload.clone();
        ensure_user_input_trace(&mut message);
        assert_eq!(message.payload, payload);
        assert!(message.trace.as_ref().unwrap().is_supported());
        assert_ne!(message.trace.as_ref().unwrap().trace_id, Uuid::nil());
    }
}
