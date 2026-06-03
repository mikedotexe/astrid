//! Native Unix socket bridge for CLI clients.
//!
//! This keeps daemon management reachable even when legacy uplink capsules fail
//! to load. It speaks the length-prefixed JSON management protocol:
//! authenticated handshake first, then `IpcMessage` frames.

use std::sync::Arc;

use astrid_core::session_token::{
    HandshakeRequest, HandshakeResponse, PROTOCOL_VERSION, SessionToken,
};
use astrid_events::ipc::{IpcMessage, IpcPayload};
use astrid_events::{AstridEvent, EventMetadata};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tracing::{debug, error, warn};

const MAX_HANDSHAKE_SIZE: usize = 4096;
const MAX_IPC_FRAME_SIZE: usize = 50 * 1024 * 1024;
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const BRIDGE_SOURCE: &str = "native_socket_bridge";

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

    publish_client_event(&kernel, IpcPayload::Connect);
    let (read_half, write_half) = stream.into_split();
    let writer = tokio::spawn(forward_events_to_client(Arc::clone(&kernel), write_half));
    read_client_messages(Arc::clone(&kernel), read_half).await;
    writer.abort();
    publish_client_event(&kernel, IpcPayload::Disconnect { reason: None });
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

async fn read_client_messages(kernel: Arc<crate::Kernel>, mut read_half: OwnedReadHalf) {
    loop {
        match read_ipc_frame(&mut read_half).await {
            Ok(Some(message)) => {
                let _ = kernel.event_bus.publish(AstridEvent::Ipc {
                    metadata: EventMetadata::new(BRIDGE_SOURCE),
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

async fn forward_events_to_client(kernel: Arc<crate::Kernel>, mut write_half: OwnedWriteHalf) {
    let mut receiver = kernel.event_bus.subscribe();
    while let Some(event) = receiver.recv().await {
        let AstridEvent::Ipc { metadata, message } = &*event else {
            continue;
        };
        if metadata.source == BRIDGE_SOURCE || message.topic.starts_with("client.v1.") {
            continue;
        }
        if let Err(e) = write_json_frame(&mut write_half, message).await {
            debug!(error = %e, "native socket bridge write failed");
            break;
        }
    }
}

fn publish_client_event(kernel: &Arc<crate::Kernel>, payload: IpcPayload) {
    let topic = match &payload {
        IpcPayload::Connect => "client.v1.connect",
        IpcPayload::Disconnect { .. } => "client.v1.disconnect",
        _ => return,
    };
    let message = IpcMessage::new(topic, payload, kernel.session_id.0);
    let _ = kernel.event_bus.publish(AstridEvent::Ipc {
        metadata: EventMetadata::new(BRIDGE_SOURCE),
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
