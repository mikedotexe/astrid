/// Run the MCP stdio server loop.
///
/// Reads JSON-RPC requests from stdin, dispatches to tool handlers,
/// and writes responses to stdout. Runs until stdin closes or shutdown
/// signal fires.
pub async fn run_mcp_server(
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> std::io::Result<()> {
    run_mcp_io(
        state,
        db,
        sensory_tx,
        shutdown,
        BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
    )
    .await
}

async fn run_mcp_io(
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    mut reader: impl tokio::io::AsyncBufRead + Unpin,
    mut stdout: impl tokio::io::AsyncWrite + Unpin,
) -> std::io::Result<()> {
    let mut line = String::new();

    info!("MCP server listening on stdio");

    loop {
        line.clear();

        tokio::select! {
            biased;
            _ = crate::lifecycle::stop_requested(&mut shutdown) => {
                info!("MCP server shutting down");
                return Ok(());
            }
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("MCP server stdin closed");
                        return Ok(());
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
                                return Err(e);
                            }
                            stdout.flush().await?;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "MCP stdin read error");
                        return Err(e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod lifecycle_io_tests {
    use super::*;
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    struct FailingOutput {
        fail_write: bool,
    }

    impl tokio::io::AsyncWrite for FailingOutput {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(if self.fail_write {
                Err(io::Error::other("write failed"))
            } else {
                Ok(bytes.len())
            })
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::other("flush failed")))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    async fn fixture(stop: bool, output: impl tokio::io::AsyncWrite + Unpin) -> io::Result<()> {
        let (_tx, rx) = tokio::sync::watch::channel(stop);
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        run_mcp_io(
            Arc::new(RwLock::new(BridgeState::new())),
            Arc::new(BridgeDb::open(":memory:").unwrap()),
            sensory_tx,
            rx,
            BufReader::new(&b"not json\n"[..]),
            output,
        )
        .await
    }

    #[tokio::test]
    async fn preexisting_shutdown_prevents_buffered_request_admission() {
        fixture(true, FailingOutput { fail_write: true })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn response_write_and_flush_failures_prevent_successful_drain() {
        assert_eq!(
            fixture(false, FailingOutput { fail_write: true })
                .await
                .unwrap_err()
                .to_string(),
            "write failed"
        );
        assert_eq!(
            fixture(false, FailingOutput { fail_write: false })
                .await
                .unwrap_err()
                .to_string(),
            "flush failed"
        );
    }
}
