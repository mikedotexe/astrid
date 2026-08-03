//! Headless and snapshot-TUI modes for non-interactive use.

use std::io::IsTerminal;

use anyhow::{Context, Result};

use super::daemon;
use crate::{formatter, socket_client, tui};

const CANONICAL_AGENT_RESPONSE_TOPIC: &str = "agent.v1.response";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";
const HEADLESS_TRACE_RECEIPT_PREFIX: &str = "[astrid-headless-trace] ";
const HEADLESS_PROVENANCE_RECEIPT_PREFIX: &str = "[astrid-headless-provenance] ";
const DEFAULT_HEADLESS_IDLE_TIMEOUT_SECONDS: u64 = 120;
const HEADLESS_IDLE_TIMEOUT_EXIT_CODE: i32 = 53;

struct CollectedResponse {
    response_text: String,
    tool_calls: Vec<serde_json::Value>,
    canonical_trace: Option<astrid_types::ipc::IpcTraceContextV1>,
    response_provenance: Option<astrid_types::ipc::AgentResponseProvenanceV1>,
}

/// Observational tracing and liveness controls supplied by a supervised
/// headless caller. These values grant no Action or capsule authority.
pub(crate) struct HeadlessControl {
    /// Optional trace root selected by the caller.
    pub(crate) trace_id: Option<uuid::Uuid>,
    /// Optional sovereign Action chain associated with the trace.
    pub(crate) trace_chain_id: Option<String>,
    /// Optional per-message idle deadline override.
    pub(crate) idle_timeout_seconds: Option<u64>,
}

/// Snapshot TUI mode: render the TUI to stdout as text frames.
///
/// Uses the same daemon connection as headless mode, but renders through
/// ratatui's `TestBackend` and dumps each significant event as a text frame.
pub(crate) async fn run_snapshot_tui(
    prompt: String,
    auto_approve: bool,
    session_name: Option<String>,
    width: u16,
    height: u16,
) -> Result<()> {
    use astrid_core::SessionId;

    daemon::ensure_daemon("snapshot-tui").await?;

    let session_id = if let Some(ref name) = session_name {
        let ns = uuid::Uuid::NAMESPACE_URL;
        SessionId::from_uuid(uuid::Uuid::new_v5(&ns, name.as_bytes()))
    } else {
        SessionId::from_uuid(uuid::Uuid::new_v4())
    };

    let mut client = socket_client::SocketClient::connect(session_id.clone())
        .await
        .context("Failed to connect to daemon")?;

    let workspace = std::env::current_dir().ok();
    tui::headless::run(tui::headless::HeadlessConfig {
        client: &mut client,
        session_id: &session_id,
        workspace,
        model_name: "",
        prompt: &prompt,
        width,
        height,
        auto_approve,
    })
    .await
}

/// Headless mode: send a single prompt, stream the response to stdout, exit.
///
/// Connects to the daemon (spawning if needed), sends the prompt as a
/// `UserInput` IPC message, and reads response events until the final
/// `AgentResponse` with `is_final = true`.
///
/// Output format:
/// - `Pretty`: prints the raw response text to stdout.
/// - `Json`: prints a JSON object with `response` and tool call details.
pub(crate) async fn run_headless(
    prompt: String,
    format: formatter::OutputFormat,
    auto_approve: bool,
    session_name: Option<String>,
    print_session: bool,
    control: HeadlessControl,
) -> Result<()> {
    use astrid_core::SessionId;

    daemon::ensure_daemon("headless").await?;

    // Use a named session (deterministic UUID v5 from name) or fresh UUID v4.
    let session_id = if let Some(ref name) = session_name {
        // Derive a stable UUID from the session name so the same name always
        // maps to the same session ID across invocations.
        let ns = uuid::Uuid::NAMESPACE_URL;
        let id = uuid::Uuid::new_v5(&ns, name.as_bytes());
        if print_session {
            eprintln!("[headless] Session: {name} ({id})");
        }
        SessionId::from_uuid(id)
    } else {
        let id = uuid::Uuid::new_v4();
        if print_session {
            eprintln!("[headless] Session: {id}");
        }
        SessionId::from_uuid(id)
    };
    let mut client = socket_client::SocketClient::connect(session_id.clone())
        .await
        .context("Failed to connect to daemon")?;

    // Also read stdin if there's piped content and -p was used
    let full_prompt = if std::io::stdin().is_terminal() {
        prompt
    } else {
        let mut stdin_text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin_text)?;
        if stdin_text.is_empty() {
            prompt
        } else {
            format!("{stdin_text}\n\n{prompt}")
        }
    };

    // Send the prompt and collect the streaming response
    let trace = control.trace_id.map(|trace_id| {
        astrid_types::ipc::IpcTraceContextV1::root(
            trace_id,
            session_id.0.to_string(),
            control.trace_chain_id,
        )
    });
    client
        .send_input_with_trace(full_prompt, trace.clone())
        .await?;
    let collected = collect_response(
        &mut client,
        &session_id,
        format,
        auto_approve,
        trace.as_ref(),
        std::time::Duration::from_secs(
            control
                .idle_timeout_seconds
                .unwrap_or(DEFAULT_HEADLESS_IDLE_TIMEOUT_SECONDS),
        ),
    )
    .await?;
    if let Some(canonical_trace) = collected.canonical_trace.as_ref() {
        eprintln!(
            "{HEADLESS_TRACE_RECEIPT_PREFIX}{}",
            serde_json::to_string(canonical_trace)?
        );
        eprintln!(
            "{HEADLESS_PROVENANCE_RECEIPT_PREFIX}{}",
            serde_json::to_string(&collected.response_provenance.context(
                "scheduled headless response ended without explicit terminal provenance"
            )?)?
        );
    }

    // Final output
    match format {
        formatter::OutputFormat::Pretty => {
            if !collected.response_text.ends_with('\n') {
                println!();
            }
        },
        formatter::OutputFormat::Json => {
            let output = serde_json::json!({
                "response": collected.response_text,
                "tool_calls": collected.tool_calls,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        },
    }

    // Send disconnect
    let disconnect = astrid_types::ipc::IpcMessage::new(
        "client.v1.disconnect",
        astrid_types::ipc::IpcPayload::Disconnect {
            reason: Some("headless".to_string()),
        },
        session_id.0,
    );
    let _ = client.send_message(disconnect).await;

    Ok(())
}

/// Collect the streaming response from the daemon in headless mode.
///
/// Returns `(response_text, tool_calls)`. Auto-denies any approval requests.
/// Uses the supplied per-message idle deadline.
async fn collect_response(
    client: &mut socket_client::SocketClient,
    session_id: &astrid_core::SessionId,
    format: formatter::OutputFormat,
    auto_approve: bool,
    expected_trace: Option<&astrid_types::ipc::IpcTraceContextV1>,
    timeout_duration: std::time::Duration,
) -> Result<CollectedResponse> {
    let mut response_text = String::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();
    let mut canonical_trace: Option<astrid_types::ipc::IpcTraceContextV1> = None;
    let mut response_provenance = None;
    loop {
        let Some(message) = read_before_idle_deadline(client, timeout_duration).await? else {
            break;
        };

        if let Some(expected) = expected_trace
            && message
                .trace
                .as_ref()
                .is_none_or(|trace| trace.trace_id != expected.trace_id)
        {
            continue;
        }

        match &message.payload {
            astrid_types::ipc::IpcPayload::AgentResponse {
                text,
                is_final,
                response_provenance: provenance,
                ..
            } => {
                if let Some(expected) = expected_trace
                    && !record_canonical_response_trace(
                        &message,
                        session_id,
                        expected,
                        &mut canonical_trace,
                    )?
                {
                    continue;
                }
                if format == formatter::OutputFormat::Pretty {
                    print!("{text}");
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                response_text.push_str(text);
                if *is_final {
                    response_provenance = *provenance;
                    break;
                }
            },
            astrid_types::ipc::IpcPayload::LlmStreamEvent {
                event: astrid_types::llm::StreamEvent::ToolCallStart { id, name },
                ..
            } => {
                tool_calls.push(serde_json::json!({
                    "type": "tool_call",
                    "id": id,
                    "name": name,
                }));
            },
            astrid_types::ipc::IpcPayload::ToolExecuteResult { call_id, result } => {
                tool_calls.push(serde_json::json!({
                    "type": "tool_result",
                    "call_id": call_id,
                    "content": result.content,
                    "is_error": result.is_error,
                }));
            },
            astrid_types::ipc::IpcPayload::ApprovalRequired {
                request_id, action, ..
            } => {
                let decision = if auto_approve { "approve" } else { "deny" };
                eprintln!(
                    "[headless] Auto-{} approval for: {action}",
                    if auto_approve { "approved" } else { "denied" }
                );
                let response = astrid_types::ipc::IpcPayload::ApprovalResponse {
                    request_id: request_id.clone(),
                    decision: decision.to_string(),
                    reason: Some(
                        if auto_approve {
                            "headless --yes mode"
                        } else {
                            "headless mode"
                        }
                        .to_string(),
                    ),
                    boundary_id: None,
                };
                let topic = format!("astrid.v1.approval.response.{request_id}");
                let msg = astrid_types::ipc::IpcMessage::new(topic, response, session_id.0);
                client.send_message(msg).await?;
            },
            _ => {},
        }
    }

    finish_collected_response(
        response_text,
        tool_calls,
        canonical_trace,
        response_provenance,
        expected_trace.is_some(),
    )
}

async fn read_before_idle_deadline(
    client: &mut socket_client::SocketClient,
    timeout_duration: std::time::Duration,
) -> Result<Option<astrid_types::ipc::IpcMessage>> {
    match tokio::time::timeout(timeout_duration, client.read_message()).await {
        Ok(Ok(message)) => Ok(message),
        Ok(Err(error)) => Err(error.context("Failed to read from daemon")),
        Err(_) => {
            eprintln!(
                "[headless] Timed out waiting for response ({}s)",
                timeout_duration.as_secs()
            );
            std::process::exit(HEADLESS_IDLE_TIMEOUT_EXIT_CODE);
        },
    }
}

fn finish_collected_response(
    response_text: String,
    tool_calls: Vec<serde_json::Value>,
    canonical_trace: Option<astrid_types::ipc::IpcTraceContextV1>,
    response_provenance: Option<astrid_types::ipc::AgentResponseProvenanceV1>,
    trace_required: bool,
) -> Result<CollectedResponse> {
    if trace_required && canonical_trace.is_none() {
        anyhow::bail!("headless response ended without a kernel-attested canonical turn trace");
    }
    reject_executor_terminal_error(response_provenance)?;
    require_scheduled_terminal_provenance(response_provenance, trace_required)?;
    Ok(CollectedResponse {
        response_text,
        tool_calls,
        canonical_trace,
        response_provenance,
    })
}

fn reject_executor_terminal_error(
    provenance: Option<astrid_types::ipc::AgentResponseProvenanceV1>,
) -> Result<()> {
    if provenance == Some(astrid_types::ipc::AgentResponseProvenanceV1::ExecutorTerminalError) {
        anyhow::bail!("executor-generated terminal response is non-authored");
    }
    Ok(())
}

fn require_scheduled_terminal_provenance(
    provenance: Option<astrid_types::ipc::AgentResponseProvenanceV1>,
    trace_required: bool,
) -> Result<()> {
    if trace_required && provenance.is_none() {
        anyhow::bail!(
            "scheduled headless response lacks explicit terminal provenance; treating it as non-authored"
        );
    }
    Ok(())
}

fn record_canonical_response_trace(
    message: &astrid_types::ipc::IpcMessage,
    session_id: &astrid_core::SessionId,
    expected: &astrid_types::ipc::IpcTraceContextV1,
    canonical_trace: &mut Option<astrid_types::ipc::IpcTraceContextV1>,
) -> Result<bool> {
    let Some(attested_trace) = canonical_response_trace(message, session_id, expected) else {
        return Ok(false);
    };
    if canonical_trace
        .as_ref()
        .is_some_and(|prior| prior.turn_id != attested_trace.turn_id)
    {
        anyhow::bail!("kernel-attested response changed turn identity within one headless request");
    }
    *canonical_trace = Some(attested_trace);
    Ok(true)
}

fn canonical_response_trace(
    message: &astrid_types::ipc::IpcMessage,
    session_id: &astrid_core::SessionId,
    expected: &astrid_types::ipc::IpcTraceContextV1,
) -> Option<astrid_types::ipc::IpcTraceContextV1> {
    if message.topic != CANONICAL_AGENT_RESPONSE_TOPIC {
        return None;
    }
    let producer = message
        .producer
        .as_ref()
        .filter(|producer| producer.is_supported())?;
    if producer.kind != "wasm_capsule" || producer.id != REACT_CAPSULE_ID {
        return None;
    }
    let astrid_types::ipc::IpcPayload::AgentResponse {
        session_id: payload_session,
        ..
    } = &message.payload
    else {
        return None;
    };
    let expected_session = session_id.0.to_string();
    if payload_session != &expected_session {
        return None;
    }
    let trace = message
        .trace
        .as_ref()
        .filter(|trace| trace.is_supported())?;
    if trace.trace_id != expected.trace_id
        || trace.turn_id.is_none()
        || trace.session_id.as_deref() != Some(expected_session.as_str())
        || trace.chain_id != expected.chain_id
    {
        return None;
    }
    Some(trace.clone())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_response_trace, record_canonical_response_trace, reject_executor_terminal_error,
        require_scheduled_terminal_provenance,
    };
    use astrid_core::SessionId;
    use astrid_types::ipc::{
        AgentResponseProvenanceV1, IpcMessage, IpcPayload, IpcProducerV1, IpcTraceContextV1,
    };

    fn response(session: &SessionId, trace: IpcTraceContextV1) -> IpcMessage {
        IpcMessage::new(
            "agent.v1.response",
            IpcPayload::AgentResponse {
                text: "NEXT: LISTEN".to_string(),
                is_final: true,
                session_id: session.0.to_string(),
                response_provenance: Some(AgentResponseProvenanceV1::ModelAuthored),
            },
            uuid::Uuid::new_v4(),
        )
        .with_trace(trace)
        .with_producer(IpcProducerV1::new("wasm_capsule", "astrid-capsule-react"))
    }

    #[test]
    fn traced_headless_response_requires_kernel_attested_exact_turn() {
        let session = SessionId::from_uuid(uuid::Uuid::new_v4());
        let expected = IpcTraceContextV1::root(
            uuid::Uuid::new_v4(),
            session.0.to_string(),
            Some("chain-one".to_string()),
        );
        let canonical = IpcTraceContextV1::root(
            expected.trace_id,
            session.0.to_string(),
            expected.chain_id.clone(),
        );
        let message = response(&session, canonical.clone());
        assert_eq!(
            canonical_response_trace(&message, &session, &expected).and_then(|trace| trace.turn_id),
            canonical.turn_id
        );

        let mut default_session = message.clone();
        if let IpcPayload::AgentResponse { session_id, .. } = &mut default_session.payload {
            *session_id = "default".to_string();
        }
        assert!(canonical_response_trace(&default_session, &session, &expected).is_none());

        let mut spoofed = message.clone();
        spoofed.producer = Some(IpcProducerV1::new(
            "native_socket_client",
            "astrid-capsule-react",
        ));
        assert!(canonical_response_trace(&spoofed, &session, &expected).is_none());

        let mut wrong_turn = message;
        wrong_turn.trace.as_mut().unwrap().trace_id = uuid::Uuid::new_v4();
        assert!(canonical_response_trace(&wrong_turn, &session, &expected).is_none());

        let mut recorded = None;
        assert!(
            record_canonical_response_trace(
                &response(&session, canonical.clone()),
                &session,
                &expected,
                &mut recorded,
            )
            .unwrap()
        );
        assert!(
            record_canonical_response_trace(
                &response(&session, canonical.child()),
                &session,
                &expected,
                &mut recorded,
            )
            .unwrap()
        );
        let conflicting_turn = IpcTraceContextV1::root(
            expected.trace_id,
            session.0.to_string(),
            expected.chain_id.clone(),
        );
        assert!(
            record_canonical_response_trace(
                &response(&session, conflicting_turn),
                &session,
                &expected,
                &mut recorded,
            )
            .is_err()
        );
    }

    #[test]
    fn scheduled_headless_turn_rejects_executor_terminal_error_provenance() {
        assert!(
            reject_executor_terminal_error(Some(AgentResponseProvenanceV1::ExecutorTerminalError))
                .is_err()
        );
        assert!(
            reject_executor_terminal_error(Some(AgentResponseProvenanceV1::ModelAuthored)).is_ok()
        );
        assert!(
            reject_executor_terminal_error(Some(
                AgentResponseProvenanceV1::ModelAuthoredWithLocalSafeFallback
            ))
            .is_ok()
        );
        assert!(reject_executor_terminal_error(None).is_ok());
    }

    #[test]
    fn scheduled_headless_turn_rejects_legacy_missing_terminal_provenance() {
        assert!(require_scheduled_terminal_provenance(None, true).is_err());
        assert!(require_scheduled_terminal_provenance(None, false).is_ok());
        assert!(
            require_scheduled_terminal_provenance(
                Some(AgentResponseProvenanceV1::ModelAuthored),
                true,
            )
            .is_ok()
        );
    }
}
