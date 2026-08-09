//! Trace-preserving IPC observers and bounded private tool gateways.
//!
//! Web, introspection, and spectral gateways share exact call/trace correlation,
//! two-phase receipts, and body-exclusion rules. They remain co-located for this
//! release so those invariants have one implementation; a later split should
//! first extract the common correlated-request transaction and sanitization API.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{BufRead as _, BufReader, Write as _},
    os::unix::fs::OpenOptionsExt as _,
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::UnixStream,
    sync::{broadcast, mpsc, watch},
};
use uuid::Uuid;

use crate::{
    actions::{
        ActionCandidate, model_authored_prefix_before_safe_fallback, transport_recovery_reason,
    },
    autonomy::is_autonomous_prompt,
    codec::encode_text,
    config::Config,
    maintenance::WorkTracker,
    notebook::ActivityEvent,
    reservoir::SensoryIngress,
    trace::{
        AutonomyTraceMatch, AutonomyTraceRegistry, IpcTraceContextV1, message_producer,
        message_trace,
    },
};

const MAX_FRAME_SIZE: usize = 50 * 1024 * 1024;
const WEB_RECEIPT_SCHEMA: &str = "astrid_edge_web_tool_receipt_v2";
const INTROSPECTION_RECEIPT_SCHEMA: &str = "astrid_edge_introspection_receipt_v1";
const SPECTRAL_RECEIPT_SCHEMA: &str = "astrid_edge_spectral_receipt_v1";
const RESEARCH_SEARCH_TIMEOUT: Duration = Duration::from_secs(120);
const INTROSPECTION_TIMEOUT: Duration = Duration::from_secs(120);
const SPECTRAL_TIMEOUT: Duration = Duration::from_secs(120);
const SOURCE_FETCH_TIMEOUT: Duration = Duration::from_secs(120);
const SOURCE_FETCH_MAX_CHARS: u64 = 64 * 1_024;
const EDGE_EXECUTOR_SOURCE_ID: &str = "a57d1d30-0000-4000-8000-000000000001";
const CANONICAL_AGENT_RESPONSE_TOPIC: &str = "agent.v1.response";
const REACT_CAPSULE_ID: &str = "astrid-capsule-react";
const EDGE_INTROSPECTOR_CAPSULE_ID: &str = "astrid-capsule-edge-introspector";
const EDGE_SPECTRAL_CAPSULE_ID: &str = "astrid-capsule-edge-spectral";
const HTTP_CAPSULE_ID: &str = "astrid-capsule-http";
const OPERATOR_INQUIRY_SESSION_SEED: &[u8] = b"edge-operator-inquiry-harness-v1";
const OPERATOR_INTROSPECTION_SESSION_SEED: &[u8] = b"edge-operator-introspection-harness-v1";
const SCHEDULED_INTROSPECTION_SESSION_SEED: &[u8] = b"edge-scheduled-introspection-v1";
const DIRECT_HEADLESS_MAX_RESPONSE_BYTES: usize = 128 * 1_024;
static WEB_CALL_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static INTROSPECTION_CALL_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static SPECTRAL_CALL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Exact result of one in-process, authenticated headless turn.
///
/// This replaces the former mutable `astrid` CLI subprocess while preserving
/// the same kernel-attested trace, terminal provenance, and host-owned
/// provider-metrics evidence.
pub(crate) struct DirectHeadlessTurn {
    pub(crate) response: String,
    pub(crate) canonical_trace: IpcTraceContextV1,
    pub(crate) response_provenance: String,
    pub(crate) provider_metrics_receipt: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectralQuery {
    Now,
    Window { minutes: u16 },
    Correlate { limit: u8 },
}

#[derive(Debug, Clone)]
pub struct PublicSourceEvidence {
    pub result_id: u8,
    pub query: String,
    pub title: String,
    pub url: String,
    pub status: u64,
    pub body: String,
    pub original_body_bytes: u64,
    pub truncated: bool,
    pub body_sha256: String,
    pub retrieved_at_unix_ms: u64,
    pub relevance_score: f64,
    pub source_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedSearchResult {
    pub(crate) query: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) relevance_score_millis: u16,
    pub(crate) source_class: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PendingWebCall {
    tool_name: String,
    arguments: Value,
    requested_at_unix_ms: u64,
    origin: String,
    parent_response_sha256: Option<String>,
    trace: Option<IpcTraceContextV1>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WebToolReceipt {
    schema: String,
    phase: String,
    recorded_at_unix_ms: u64,
    requested_at_unix_ms: u64,
    completed_at_unix_ms: Option<u64>,
    latency_ms: Option<u64>,
    call_id: String,
    tool_name: String,
    arguments: Value,
    status: String,
    result_summary: Option<Value>,
    result_sha256: Option<String>,
    source_topic: String,
    origin: String,
    parent_response_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<IpcTraceContextV1>,
    authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct IntrospectionReceipt {
    schema: String,
    phase: String,
    recorded_at_unix_ms: u64,
    requested_at_unix_ms: u64,
    completed_at_unix_ms: Option<u64>,
    latency_ms: Option<u64>,
    call_id: String,
    tool_name: String,
    arguments: Value,
    status: String,
    result_summary: Option<Value>,
    result_sha256: Option<String>,
    source_topic: String,
    origin: String,
    parent_response_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<IpcTraceContextV1>,
    authority: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SpectralReceipt {
    schema: String,
    phase: String,
    recorded_at_unix_ms: u64,
    requested_at_unix_ms: u64,
    completed_at_unix_ms: Option<u64>,
    latency_ms: Option<u64>,
    call_id: String,
    tool_name: String,
    arguments: Value,
    status: String,
    result_summary: Option<Value>,
    result_sha256: Option<String>,
    source_topic: String,
    origin: String,
    parent_response_sha256: String,
    trace: IpcTraceContextV1,
    authority: String,
}

/// Accept a private capsule result only when the kernel-attested producer,
/// exact result topic, call identity, and one-hop causal lineage all match the
/// request. Call IDs are observable routing labels, not authentication.
fn verified_capsule_tool_result(
    message: &Value,
    call_id: &str,
    tool_name: &str,
    capsule_id: &str,
    request_trace: &IpcTraceContextV1,
) -> Option<IpcTraceContextV1> {
    let expected_topic = format!("tool.v1.execute.{tool_name}.result");
    if message.get("topic").and_then(Value::as_str) != Some(expected_topic.as_str()) {
        return None;
    }
    let payload = message.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("tool_execute_result")
        || payload.get("call_id").and_then(Value::as_str) != Some(call_id)
    {
        return None;
    }
    let producer = message_producer(message)?;
    if producer.kind != "wasm_capsule" || producer.id != capsule_id {
        return None;
    }
    let result_trace = message_trace(message)?;
    (result_trace.trace_id == request_trace.trace_id
        && result_trace.turn_id == request_trace.turn_id
        && result_trace.parent_span_id == Some(request_trace.span_id)
        && result_trace.session_id == request_trace.session_id
        && result_trace.chain_id == request_trace.chain_id)
        .then_some(result_trace)
}

/// Execute one model-hidden, read-only edge-spectral capsule query.
///
/// The caller must supply the exact trace and terminal authored response hash
/// that declared the voluntary Action. This helper neither infers correlation
/// from timestamps nor grants control authority. Its durable receipt contains
/// only bounded sanitized numeric/identifier summaries and a hash of the
/// returned body, never the raw returned body.
#[allow(clippy::too_many_lines)] // Request/result provenance stays visibly paired.
pub async fn execute_spectral_query(
    config: &Config,
    query: SpectralQuery,
    parent_trace: &IpcTraceContextV1,
    parent_response_sha256: &str,
    origin: &str,
) -> Result<Value> {
    if !parent_trace.is_supported() {
        bail!("spectral query requires a supported exact trace");
    }
    if parent_response_sha256.len() != 64
        || !parent_response_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("spectral query requires an exact 64-character response hash");
    }
    let (tool_name, arguments) =
        spectral_tool_arguments(query, parent_trace, parent_response_sha256)?;
    let call_id = format!(
        "edge-spectral-{}-{}",
        unix_millis(),
        SPECTRAL_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let trace = parent_trace.child();
    let requested_at_unix_ms = unix_millis();
    let requested = SpectralReceipt {
        schema: SPECTRAL_RECEIPT_SCHEMA.to_string(),
        phase: "requested".to_string(),
        recorded_at_unix_ms: requested_at_unix_ms,
        requested_at_unix_ms,
        completed_at_unix_ms: None,
        latency_ms: None,
        call_id: call_id.clone(),
        tool_name: tool_name.to_string(),
        arguments: arguments.clone(),
        status: "requested".to_string(),
        result_summary: None,
        result_sha256: None,
        source_topic: format!("tool.v1.execute.{tool_name}"),
        origin: bounded_text(origin, 80),
        parent_response_sha256: parent_response_sha256.to_ascii_lowercase(),
        trace: trace.clone(),
        authority: "private_read_only_spectral_request_not_model_authorship_or_control".to_string(),
    };
    append_spectral_receipt(config, &requested)?;

    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    write_frame(
        &mut stream,
        &spectral_tool_request(&call_id, tool_name, &arguments, Some(&trace)),
    )
    .await?;

    let (result_message, result_trace) = tokio::time::timeout(SPECTRAL_TIMEOUT, async {
        loop {
            let message = read_frame(&mut stream).await?;
            if let Some(result_trace) = verified_capsule_tool_result(
                &message,
                &call_id,
                tool_name,
                EDGE_SPECTRAL_CAPSULE_ID,
                &trace,
            ) {
                return Ok::<(Value, IpcTraceContextV1), anyhow::Error>((message, result_trace));
            }
        }
    })
    .await
    .with_context(|| format!("private spectral query timed out for {call_id}"))??;
    let payload = result_message.get("payload").unwrap_or(&Value::Null);
    let result = payload.get("result").unwrap_or(&Value::Null);
    let content = result
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if content.chars().count() > 24_000 {
        bail!("private spectral result exceeded the bounded result limit");
    }
    let is_error = result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let parsed = serde_json::from_str::<Value>(content).unwrap_or(Value::Null);
    let completed_at_unix_ms = unix_millis();
    let completed = SpectralReceipt {
        schema: SPECTRAL_RECEIPT_SCHEMA.to_string(),
        phase: "completed".to_string(),
        recorded_at_unix_ms: completed_at_unix_ms,
        requested_at_unix_ms,
        completed_at_unix_ms: Some(completed_at_unix_ms),
        latency_ms: Some(completed_at_unix_ms.saturating_sub(requested_at_unix_ms)),
        call_id: call_id.clone(),
        tool_name: tool_name.to_string(),
        arguments,
        status: if is_error { "error" } else { "success" }.to_string(),
        result_summary: Some(summarize_spectral_result(
            tool_name, &parsed, content, is_error,
        )),
        result_sha256: Some(format!("{:x}", Sha256::digest(content.as_bytes()))),
        source_topic: format!("tool.v1.execute.{tool_name}.result"),
        origin: bounded_text(origin, 80),
        parent_response_sha256: parent_response_sha256.to_ascii_lowercase(),
        trace: result_trace,
        authority: "verified_private_spectral_result_not_model_authorship_or_causal_proof"
            .to_string(),
    };
    append_spectral_receipt(config, &completed)?;
    if is_error {
        bail!(
            "private spectral query returned an error: {}",
            bounded_text(content, 300)
        );
    }
    Ok(parsed)
}

#[allow(clippy::too_many_lines)] // One two-phase call keeps request/result provenance atomic.
pub async fn execute_introspection_search(
    config: &Config,
    question: &str,
    parent_trace: Option<&IpcTraceContextV1>,
    parent_response_sha256: Option<&str>,
    origin: &str,
) -> Result<Value> {
    validate_introspection_origin(origin, parent_trace, parent_response_sha256)?;
    let query = bounded_text(question.trim(), 160);
    if query.is_empty() {
        bail!("self-study question is empty");
    }
    let call_id = format!(
        "edge-introspection-{}-{}",
        unix_millis(),
        INTROSPECTION_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let trace = parent_trace
        .context("private introspection requires a supported exact trace")?
        .child();
    let requested_at_unix_ms = unix_millis();
    let arguments = json!({"question": query, "limit": 8});
    let requested = IntrospectionReceipt {
        schema: INTROSPECTION_RECEIPT_SCHEMA.to_string(),
        phase: "requested".to_string(),
        recorded_at_unix_ms: requested_at_unix_ms,
        requested_at_unix_ms,
        completed_at_unix_ms: None,
        latency_ms: None,
        call_id: call_id.clone(),
        tool_name: "inspect_owned_question".to_string(),
        arguments: arguments.clone(),
        status: "requested".to_string(),
        result_summary: None,
        result_sha256: None,
        source_topic: "tool.v1.execute.inspect_owned_question".to_string(),
        origin: bounded_text(origin, 80),
        parent_response_sha256: parent_response_sha256.map(ToOwned::to_owned),
        trace: Some(trace.clone()),
        authority: "private_read_only_introspection_request_not_model_authorship".to_string(),
    };
    append_introspection_receipt(config, &requested)?;

    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    write_frame(
        &mut stream,
        &introspection_search_request(&call_id, &query, Some(&trace)),
    )
    .await?;
    eprintln!(
        "private SELF_STUDY search dispatched: call_id={call_id} query_chars={}",
        query.chars().count()
    );

    let (result_message, result_trace) = tokio::time::timeout(INTROSPECTION_TIMEOUT, async {
        loop {
            let message = read_frame(&mut stream).await?;
            if let Some(result_trace) = verified_capsule_tool_result(
                &message,
                &call_id,
                "inspect_owned_question",
                EDGE_INTROSPECTOR_CAPSULE_ID,
                &trace,
            ) {
                return Ok::<(Value, IpcTraceContextV1), anyhow::Error>((message, result_trace));
            }
        }
    })
    .await
    .with_context(|| format!("private introspection timed out for {call_id}"))??;
    let result = result_message.get("payload").unwrap_or(&Value::Null);

    let content = result
        .get("result")
        .and_then(|value| value.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let is_error = result
        .get("result")
        .and_then(|value| value.get("is_error"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let completed_at_unix_ms = unix_millis();
    let parsed = serde_json::from_str::<Value>(content).unwrap_or(Value::Null);
    let completed = IntrospectionReceipt {
        schema: INTROSPECTION_RECEIPT_SCHEMA.to_string(),
        phase: "completed".to_string(),
        recorded_at_unix_ms: completed_at_unix_ms,
        requested_at_unix_ms,
        completed_at_unix_ms: Some(completed_at_unix_ms),
        latency_ms: Some(completed_at_unix_ms.saturating_sub(requested_at_unix_ms)),
        call_id: call_id.clone(),
        tool_name: "inspect_owned_question".to_string(),
        arguments,
        status: if is_error { "error" } else { "success" }.to_string(),
        result_summary: Some(summarize_introspection_result(&parsed, content, is_error)),
        result_sha256: Some(format!("{:x}", Sha256::digest(content.as_bytes()))),
        source_topic: "tool.v1.execute.inspect_owned_question.result".to_string(),
        origin: bounded_text(origin, 80),
        parent_response_sha256: parent_response_sha256.map(ToOwned::to_owned),
        trace: Some(result_trace),
        authority: "verified_private_read_only_result_not_astrid_authorship".to_string(),
    };
    append_introspection_receipt(config, &completed)?;
    if is_error {
        bail!(
            "private introspection returned an error: {}",
            bounded_text(content, 300)
        );
    }
    eprintln!("private SELF_STUDY search completed: call_id={call_id}");
    Ok(parsed)
}

fn validate_introspection_origin(
    origin: &str,
    parent_trace: Option<&IpcTraceContextV1>,
    parent_response_sha256: Option<&str>,
) -> Result<()> {
    let trace = parent_trace.context("private introspection requires a supported exact trace")?;
    if !trace.is_supported() {
        bail!("private introspection requires a supported exact trace");
    }
    match origin {
        "action_executor_self_study" => {
            let response_sha256 = parent_response_sha256
                .context("authored SELF_STUDY requires its exact parent response hash")?;
            if response_sha256.len() != 64
                || !response_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                bail!("authored SELF_STUDY requires an exact 64-character response hash");
            }
        },
        "operator_harness" => {
            if trace.session_id.as_deref() != Some(operator_introspection_session_id().as_str())
                || parent_response_sha256.is_some()
            {
                bail!("operator introspection harness trace/provenance is invalid");
            }
        },
        "scheduled_introspection_prefetch" => {
            if trace.session_id.as_deref() != Some(scheduled_introspection_session_id().as_str())
                || parent_response_sha256.is_some()
            {
                bail!("scheduled introspection prefetch trace/provenance is invalid");
            }
        },
        _ => bail!("unsupported private introspection origin"),
    }
    Ok(())
}

pub async fn execute_research_search(
    config: &Config,
    question: &str,
    parent_trace: Option<&IpcTraceContextV1>,
    parent_response_sha256: Option<&str>,
) -> Result<()> {
    let original_query = bounded_text(question.trim(), 300);
    if original_query.is_empty() {
        bail!("research question is empty");
    }
    let first_content = execute_one_research_search(
        config,
        &original_query,
        parent_trace,
        parent_response_sha256,
        1,
    )
    .await?;
    if search_has_useful_result(&first_content, &original_query) {
        return Ok(());
    }
    let contextual_query = contextualized_research_query(config, &original_query);
    if contextual_query == original_query {
        eprintln!("sovereign RESEARCH found no useful evidence; no distinct contextual retry");
        return Ok(());
    }
    let second_content = execute_one_research_search(
        config,
        &contextual_query,
        parent_trace,
        parent_response_sha256,
        2,
    )
    .await?;
    if !search_has_useful_result(&second_content, &original_query) {
        eprintln!("sovereign RESEARCH completed two attempts with no useful evidence");
    }
    Ok(())
}

async fn execute_one_research_search(
    config: &Config,
    query: &str,
    parent_trace: Option<&IpcTraceContextV1>,
    parent_response_sha256: Option<&str>,
    attempt: u8,
) -> Result<String> {
    let call_id = format!(
        "edge-research-{attempt}-{}-{}",
        unix_millis(),
        WEB_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let trace = parent_trace.map(IpcTraceContextV1::child);
    let pending = PendingWebCall {
        tool_name: "search_web".to_string(),
        arguments: sanitized_web_arguments("search_web", &json!({"query": query, "count": 5})),
        requested_at_unix_ms: unix_millis(),
        origin: "action_executor_research".to_string(),
        parent_response_sha256: parent_response_sha256.map(ToOwned::to_owned),
        trace: trace.clone(),
    };
    append_web_receipt(config, &requested_web_receipt(&call_id, &pending))?;
    if config.web_broker_socket_path.is_some() {
        return execute_immutable_broker_search(config, &call_id, &pending, query).await;
    }
    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    let request = research_search_request(&call_id, query, trace.as_ref());
    write_frame(&mut stream, &request).await?;
    eprintln!(
        "sovereign RESEARCH dispatched bounded read-only search: call_id={call_id} query_chars={}",
        query.chars().count()
    );

    let request_trace = trace
        .as_ref()
        .context("IPC web fallback requires an exact request trace")?;
    let result_message = tokio::time::timeout(RESEARCH_SEARCH_TIMEOUT, async {
        loop {
            let message = read_frame(&mut stream).await?;
            if verified_capsule_tool_result(
                &message,
                &call_id,
                "search_web",
                HTTP_CAPSULE_ID,
                request_trace,
            )
            .is_some()
            {
                return Ok::<Value, anyhow::Error>(message);
            }
        }
    })
    .await
    .with_context(|| format!("read-only research search timed out for {call_id}"))??;
    let result = result_message.get("payload").unwrap_or(&Value::Null);

    let is_error = result
        .get("result")
        .and_then(|value| value.get("is_error"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let content = result
        .get("result")
        .and_then(|value| value.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if is_error {
        let message = result
            .get("result")
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            .unwrap_or("unknown search error");
        bail!("read-only research search returned an error: {message}");
    }
    eprintln!("sovereign RESEARCH read-only search completed: call_id={call_id}");
    Ok(content.to_string())
}

#[allow(clippy::too_many_lines)] // Request, bounded result, and provenance remain one audited call.
pub async fn execute_source_fetch(
    config: &Config,
    result_id: u8,
    parent_trace: Option<&IpcTraceContextV1>,
    parent_response_sha256: Option<&str>,
) -> Result<PublicSourceEvidence> {
    let receipt_lines = tokio::fs::read_to_string(config.workspace.join("web/receipts.jsonl"))
        .await
        .context("read completed web receipt ledger")?;
    let selected = latest_search_result(&receipt_lines, result_id)?;
    if selected
        .url
        .to_ascii_lowercase()
        .split(['?', '#'])
        .next()
        .is_some_and(|url| {
            Path::new(url)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        })
    {
        bail!("PDF sources remain explicitly unsupported on CPU-edge appliances");
    }
    let call_id = format!(
        "edge-read-source-{}-{}",
        unix_millis(),
        WEB_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let trace = parent_trace.map(IpcTraceContextV1::child);
    let pending = PendingWebCall {
        tool_name: "fetch_url".to_string(),
        arguments: sanitized_web_arguments(
            "fetch_url",
            &json!({
                "url": selected.url,
                "method": "GET",
                "max_chars": SOURCE_FETCH_MAX_CHARS
            }),
        ),
        requested_at_unix_ms: unix_millis(),
        origin: "action_executor_read_source".to_string(),
        parent_response_sha256: parent_response_sha256.map(ToOwned::to_owned),
        trace: trace.clone(),
    };
    append_web_receipt(config, &requested_web_receipt(&call_id, &pending))?;
    if config.web_broker_socket_path.is_some() {
        return execute_immutable_broker_fetch(config, &call_id, &pending, result_id, selected)
            .await;
    }
    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    let request = source_fetch_request(&call_id, &selected.url, trace.as_ref());
    write_frame(&mut stream, &request).await?;
    eprintln!(
        "sovereign READ_SOURCE dispatched bounded read-only fetch: \
         call_id={call_id} result_id={result_id}"
    );

    let request_trace = trace
        .as_ref()
        .context("IPC source fallback requires an exact request trace")?;
    let result_message = tokio::time::timeout(SOURCE_FETCH_TIMEOUT, async {
        loop {
            let message = read_frame(&mut stream).await?;
            if verified_capsule_tool_result(
                &message,
                &call_id,
                "fetch_url",
                HTTP_CAPSULE_ID,
                request_trace,
            )
            .is_some()
            {
                return Ok::<Value, anyhow::Error>(message);
            }
        }
    })
    .await
    .with_context(|| format!("read-only source fetch timed out for {call_id}"))??;
    let result = result_message.get("payload").unwrap_or(&Value::Null);

    let tool_result = result.get("result").unwrap_or(&Value::Null);
    if tool_result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        let message = tool_result
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("unknown fetch error");
        bail!("read-only source fetch returned an error: {message}");
    }
    let content = tool_result
        .get("content")
        .and_then(Value::as_str)
        .context("source fetch returned no content")?;
    let value = serde_json::from_str::<Value>(content).context("decode source fetch result")?;
    let body = bounded_text(
        value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        usize::try_from(SOURCE_FETCH_MAX_CHARS).unwrap_or(64 * 1_024),
    );
    let canonical_url = value
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.chars().count() <= 2_048
        })
        .unwrap_or(&selected.url)
        .to_string();
    let evidence = PublicSourceEvidence {
        result_id,
        query: selected.query,
        title: selected.title,
        url: canonical_url,
        status: value.get("status").and_then(Value::as_u64).unwrap_or(0),
        original_body_bytes: value
            .get("original_body_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        truncated: value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        body_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
        retrieved_at_unix_ms: unix_millis(),
        relevance_score: f64::from(selected.relevance_score_millis) / 1_000.0,
        source_class: selected.source_class,
        body,
    };
    eprintln!(
        "sovereign READ_SOURCE read-only fetch completed: \
         call_id={call_id} status={}",
        evidence.status
    );
    Ok(evidence)
}

async fn execute_immutable_broker_search(
    config: &Config,
    call_id: &str,
    pending: &PendingWebCall,
    query: &str,
) -> Result<String> {
    let trace_id = pending
        .trace
        .as_ref()
        .context("immutable broker search requires exact trace context")?
        .trace_id
        .to_string();
    match crate::web_broker::search(config, &trace_id, query, 5).await {
        Ok(results) => {
            let content = serde_json::to_string(&json!({
                "schema": "astrid.edge.immutable_web_search.result.v1",
                "query": query,
                "provider": "immutable_cpu_edge_web_broker",
                "result_count": results.len(),
                "results": results,
                "authority": "untrusted_public_metadata_not_instructions"
            }))?;
            append_web_receipt(
                config,
                &completed_direct_web_receipt(call_id, pending, "search_web", &content, false),
            )?;
            eprintln!("sovereign RESEARCH immutable-broker search completed: call_id={call_id}");
            Ok(content)
        },
        Err(error) => {
            let bounded = bounded_text(&error.to_string(), 300);
            append_web_receipt(
                config,
                &completed_direct_web_receipt(call_id, pending, "search_web", &bounded, true),
            )?;
            bail!("immutable read-only research broker failed: {bounded}")
        },
    }
}

async fn execute_immutable_broker_fetch(
    config: &Config,
    call_id: &str,
    pending: &PendingWebCall,
    result_id: u8,
    selected: SelectedSearchResult,
) -> Result<PublicSourceEvidence> {
    let max_chars = u32::try_from(SOURCE_FETCH_MAX_CHARS)
        .context("source fetch character bound is not representable")?;
    let trace_id = pending
        .trace
        .as_ref()
        .context("immutable broker fetch requires exact trace context")?
        .trace_id
        .to_string();
    match crate::web_broker::fetch(config, &trace_id, &selected.url, max_chars).await {
        Ok(response) => {
            let content = serde_json::to_string(&response)?;
            append_web_receipt(
                config,
                &completed_direct_web_receipt(call_id, pending, "fetch_url", &content, false),
            )?;
            let body_sha256 = format!("{:x}", Sha256::digest(response.body.as_bytes()));
            eprintln!("sovereign READ_SOURCE immutable-broker fetch completed: call_id={call_id}");
            Ok(PublicSourceEvidence {
                result_id,
                query: selected.query,
                title: selected.title,
                url: response.url,
                status: u64::from(response.status),
                body: response.body,
                original_body_bytes: response.original_body_bytes,
                truncated: response.truncated,
                body_sha256,
                retrieved_at_unix_ms: unix_millis(),
                relevance_score: f64::from(selected.relevance_score_millis) / 1_000.0,
                source_class: selected.source_class,
            })
        },
        Err(error) => {
            let bounded = bounded_text(&error.to_string(), 300);
            append_web_receipt(
                config,
                &completed_direct_web_receipt(call_id, pending, "fetch_url", &bounded, true),
            )?;
            bail!("immutable read-only source broker failed: {bounded}")
        },
    }
}

/// Exercise the production search/ranking path without touching Astrid's
/// workspace, continuity, reservoir, or authorship ledgers.
pub(crate) async fn execute_operator_inquiry_search(
    config: &Config,
    question: &str,
    receipt_path: &Path,
    trace: &IpcTraceContextV1,
) -> Result<Vec<SelectedSearchResult>> {
    let original_query = bounded_text(question.trim(), 240);
    if original_query.is_empty() {
        bail!("operator inquiry question is empty");
    }
    let contextual_query = bounded_text(
        &format!("{original_query} technical paper documentation architecture"),
        300,
    );
    let mut candidates = Vec::new();
    for (attempt, query) in [original_query.as_str(), contextual_query.as_str()]
        .into_iter()
        .enumerate()
    {
        if attempt == 1
            && candidates
                .iter()
                .any(|candidate: &SelectedSearchResult| candidate.relevance_score_millis >= 120)
        {
            break;
        }
        let call_id = format!(
            "edge-operator-inquiry-search-{}-{}",
            unix_millis(),
            WEB_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let call_trace = trace.child();
        let pending = PendingWebCall {
            tool_name: "search_web".to_string(),
            arguments: sanitized_web_arguments("search_web", &json!({"query": query, "count": 5})),
            requested_at_unix_ms: unix_millis(),
            origin: "operator_inquiry_harness".to_string(),
            parent_response_sha256: None,
            trace: Some(call_trace.clone()),
        };
        let content = if config.web_broker_socket_path.is_some() {
            execute_operator_broker_search(config, &call_id, &pending, query, receipt_path).await?
        } else {
            let request = research_search_request(&call_id, query, Some(&call_trace));
            execute_operator_web_call(
                config,
                &call_id,
                pending,
                request,
                RESEARCH_SEARCH_TIMEOUT,
                receipt_path,
            )
            .await?
        };
        let value = serde_json::from_str::<Value>(&content)
            .context("decode operator inquiry search result")?;
        for ranked in ranked_search_results(&value, &original_query) {
            let Some(title) = ranked.get("title").and_then(Value::as_str) else {
                continue;
            };
            let Some(url) = ranked.get("url").and_then(Value::as_str) else {
                continue;
            };
            if candidates.iter().any(|candidate| candidate.url == url) {
                continue;
            }
            candidates.push(SelectedSearchResult {
                query: original_query.clone(),
                title: title.to_string(),
                url: url.to_string(),
                relevance_score_millis: ranked
                    .get("relevance_score_millis")
                    .and_then(Value::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or_default(),
                source_class: ranked
                    .get("source_class")
                    .and_then(Value::as_str)
                    .unwrap_or("general_web")
                    .to_string(),
            });
        }
    }
    candidates.sort_by(|left, right| {
        right
            .relevance_score_millis
            .cmp(&left.relevance_score_millis)
            .then_with(|| left.url.cmp(&right.url))
    });
    candidates.truncate(3);
    Ok(candidates)
}

pub(crate) fn operator_inquiry_session_id() -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, OPERATOR_INQUIRY_SESSION_SEED).to_string()
}

pub(crate) fn operator_introspection_session_id() -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, OPERATOR_INTROSPECTION_SESSION_SEED).to_string()
}

pub(crate) fn scheduled_introspection_session_id() -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, SCHEDULED_INTROSPECTION_SESSION_SEED).to_string()
}

/// Fetch one exact candidate selected by the isolated operator harness.
pub(crate) async fn execute_operator_inquiry_fetch(
    config: &Config,
    selected: &SelectedSearchResult,
    result_id: u8,
    receipt_path: &Path,
    trace: &IpcTraceContextV1,
) -> Result<PublicSourceEvidence> {
    if selected
        .url
        .to_ascii_lowercase()
        .split(['?', '#'])
        .next()
        .is_some_and(|url| {
            Path::new(url)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        })
    {
        bail!("PDF sources remain explicitly unsupported on CPU-edge appliances");
    }
    let call_id = format!(
        "edge-operator-inquiry-fetch-{}-{}",
        unix_millis(),
        WEB_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let call_trace = trace.child();
    let pending = PendingWebCall {
        tool_name: "fetch_url".to_string(),
        arguments: sanitized_web_arguments(
            "fetch_url",
            &json!({
                "url": selected.url,
                "method": "GET",
                "max_chars": SOURCE_FETCH_MAX_CHARS
            }),
        ),
        requested_at_unix_ms: unix_millis(),
        origin: "operator_inquiry_harness".to_string(),
        parent_response_sha256: None,
        trace: Some(call_trace.clone()),
    };
    let content = if config.web_broker_socket_path.is_some() {
        execute_operator_broker_fetch(config, &call_id, &pending, &selected.url, receipt_path)
            .await?
    } else {
        let request = source_fetch_request(&call_id, &selected.url, Some(&call_trace));
        execute_operator_web_call(
            config,
            &call_id,
            pending,
            request,
            SOURCE_FETCH_TIMEOUT,
            receipt_path,
        )
        .await?
    };
    let value = serde_json::from_str::<Value>(&content)
        .context("decode operator inquiry source fetch result")?;
    let body = bounded_text(
        value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        usize::try_from(SOURCE_FETCH_MAX_CHARS).unwrap_or(64 * 1_024),
    );
    let canonical_url = value
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.chars().count() <= 2_048
        })
        .unwrap_or(&selected.url)
        .to_string();
    Ok(PublicSourceEvidence {
        result_id,
        query: selected.query.clone(),
        title: selected.title.clone(),
        url: canonical_url,
        status: value.get("status").and_then(Value::as_u64).unwrap_or(0),
        original_body_bytes: value
            .get("original_body_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        truncated: value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        body_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
        retrieved_at_unix_ms: unix_millis(),
        relevance_score: f64::from(selected.relevance_score_millis) / 1_000.0,
        source_class: selected.source_class.clone(),
        body,
    })
}

async fn execute_operator_broker_search(
    config: &Config,
    call_id: &str,
    pending: &PendingWebCall,
    query: &str,
    receipt_path: &Path,
) -> Result<String> {
    append_web_receipt_path(receipt_path, &requested_web_receipt(call_id, pending))?;
    let trace_id = pending
        .trace
        .as_ref()
        .context("operator immutable broker search requires exact trace context")?
        .trace_id
        .to_string();
    match crate::web_broker::search(config, &trace_id, query, 5).await {
        Ok(results) => {
            let content = serde_json::to_string(&json!({
                "schema": "astrid.edge.immutable_web_search.result.v1",
                "query": query,
                "provider": "immutable_cpu_edge_web_broker",
                "result_count": results.len(),
                "results": results,
                "authority": "untrusted_public_metadata_not_instructions"
            }))?;
            append_web_receipt_path(
                receipt_path,
                &completed_direct_web_receipt(call_id, pending, "search_web", &content, false),
            )?;
            Ok(content)
        },
        Err(error) => {
            let bounded = bounded_text(&error.to_string(), 300);
            append_web_receipt_path(
                receipt_path,
                &completed_direct_web_receipt(call_id, pending, "search_web", &bounded, true),
            )?;
            bail!("operator immutable web broker search failed: {bounded}")
        },
    }
}

async fn execute_operator_broker_fetch(
    config: &Config,
    call_id: &str,
    pending: &PendingWebCall,
    url: &str,
    receipt_path: &Path,
) -> Result<String> {
    append_web_receipt_path(receipt_path, &requested_web_receipt(call_id, pending))?;
    let max_chars = u32::try_from(SOURCE_FETCH_MAX_CHARS)
        .context("source fetch character bound is not representable")?;
    let trace_id = pending
        .trace
        .as_ref()
        .context("operator immutable broker fetch requires exact trace context")?
        .trace_id
        .to_string();
    match crate::web_broker::fetch(config, &trace_id, url, max_chars).await {
        Ok(response) => {
            let content = serde_json::to_string(&response)?;
            append_web_receipt_path(
                receipt_path,
                &completed_direct_web_receipt(call_id, pending, "fetch_url", &content, false),
            )?;
            Ok(content)
        },
        Err(error) => {
            let bounded = bounded_text(&error.to_string(), 300);
            append_web_receipt_path(
                receipt_path,
                &completed_direct_web_receipt(call_id, pending, "fetch_url", &bounded, true),
            )?;
            bail!("operator immutable web broker fetch failed: {bounded}")
        },
    }
}

async fn execute_operator_web_call(
    config: &Config,
    call_id: &str,
    pending: PendingWebCall,
    request: Value,
    call_timeout: Duration,
    receipt_path: &Path,
) -> Result<String> {
    append_web_receipt_path(receipt_path, &requested_web_receipt(call_id, &pending))?;
    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    write_frame(&mut stream, &request).await?;
    let request_trace = pending
        .trace
        .as_ref()
        .context("operator IPC web fallback requires an exact request trace")?;
    let result_message = tokio::time::timeout(call_timeout, async {
        loop {
            let message = read_frame(&mut stream).await?;
            if verified_capsule_tool_result(
                &message,
                call_id,
                &pending.tool_name,
                HTTP_CAPSULE_ID,
                request_trace,
            )
            .is_some()
            {
                return Ok::<Value, anyhow::Error>(message);
            }
        }
    })
    .await
    .with_context(|| format!("operator inquiry web call timed out for {call_id}"))??;
    let payload = result_message.get("payload").unwrap_or(&Value::Null);
    let mut pending_calls = HashMap::from([(call_id.to_string(), pending)]);
    let mut completed_calls = HashSet::new();
    let receipt = completed_web_receipt(
        &result_message,
        payload,
        &mut pending_calls,
        &mut completed_calls,
    )
    .context("operator inquiry result could not be correlated")?;
    append_web_receipt_path(receipt_path, &receipt)?;
    let result = payload.get("result").unwrap_or(&Value::Null);
    let content = result
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        bail!(
            "operator inquiry web call failed: {}",
            bounded_text(content, 300)
        );
    }
    Ok(content.to_string())
}

fn research_search_request(call_id: &str, query: &str, trace: Option<&IpcTraceContextV1>) -> Value {
    json!({
        "topic": "tool.v1.execute.search_web",
        "payload": {
            "type": "tool_execute_request",
            "call_id": call_id,
            "tool_name": "search_web",
            "arguments": {
                "query": query,
                "count": 5
            }
        },
        "signature": null,
        "source_id": EDGE_EXECUTOR_SOURCE_ID,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "seq": 0,
        "trace": trace
    })
}

fn introspection_search_request(
    call_id: &str,
    query: &str,
    trace: Option<&IpcTraceContextV1>,
) -> Value {
    json!({
        "topic": "tool.v1.execute.inspect_owned_question",
        "payload": {
            "type": "tool_execute_request",
            "call_id": call_id,
            "tool_name": "inspect_owned_question",
            "arguments": {
                "question": query,
                "limit": 8
            }
        },
        "signature": null,
        "source_id": EDGE_EXECUTOR_SOURCE_ID,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "seq": 0,
        "trace": trace
    })
}

fn spectral_tool_arguments(
    query: SpectralQuery,
    trace: &IpcTraceContextV1,
    parent_response_sha256: &str,
) -> Result<(&'static str, Value)> {
    match query {
        SpectralQuery::Now => Ok(("read_spectral_now", json!({}))),
        SpectralQuery::Window { minutes } if matches!(minutes, 15 | 60 | 360 | 1_440) => {
            Ok(("read_spectral_window", json!({"minutes": minutes})))
        },
        SpectralQuery::Window { .. } => {
            bail!("spectral window must be 15, 60, 360, or 1440 minutes")
        },
        SpectralQuery::Correlate { limit } if (1..=20).contains(&limit) => {
            let mut arguments = serde_json::Map::new();
            arguments.insert("trace_id".to_string(), json!(trace.trace_id));
            if let Some(session_id) = trace.session_id.as_deref() {
                ensure_spectral_identifier("session_id", session_id)?;
                arguments.insert("session_id".to_string(), json!(session_id));
            }
            if let Some(chain_id) = trace.chain_id.as_deref() {
                ensure_spectral_identifier("chain_id", chain_id)?;
                arguments.insert("chain_id".to_string(), json!(chain_id));
            }
            arguments.insert(
                "response_sha256".to_string(),
                json!(parent_response_sha256.to_ascii_lowercase()),
            );
            arguments.insert("limit".to_string(), json!(limit));
            Ok(("correlate_spectral_activity", Value::Object(arguments)))
        },
        SpectralQuery::Correlate { .. } => {
            bail!("spectral correlation result limit must be between 1 and 20")
        },
    }
}

fn ensure_spectral_identifier(name: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.chars().count() > 96 || value.chars().any(char::is_control) {
        bail!("spectral correlation {name} must be 1..=96 non-control characters");
    }
    Ok(())
}

fn spectral_tool_request(
    call_id: &str,
    tool_name: &str,
    arguments: &Value,
    trace: Option<&IpcTraceContextV1>,
) -> Value {
    debug_assert!(matches!(
        tool_name,
        "read_spectral_now" | "read_spectral_window" | "correlate_spectral_activity"
    ));
    json!({
        "topic": format!("tool.v1.execute.{tool_name}"),
        "payload": {
            "type": "tool_execute_request",
            "call_id": call_id,
            "tool_name": tool_name,
            "arguments": arguments,
        },
        "signature": null,
        "source_id": EDGE_EXECUTOR_SOURCE_ID,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "seq": 0,
        "trace": trace,
    })
}

fn source_fetch_request(call_id: &str, url: &str, trace: Option<&IpcTraceContextV1>) -> Value {
    json!({
        "topic": "tool.v1.execute.fetch_url",
        "payload": {
            "type": "tool_execute_request",
            "call_id": call_id,
            "tool_name": "fetch_url",
            "arguments": {
                "url": url,
                "method": "GET",
                "max_chars": SOURCE_FETCH_MAX_CHARS
            }
        },
        "signature": null,
        "source_id": EDGE_EXECUTOR_SOURCE_ID,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "seq": 0,
        "trace": trace
    })
}

fn latest_search_result(receipt_lines: &str, result_id: u8) -> Result<SelectedSearchResult> {
    let result_index = usize::from(
        result_id
            .checked_sub(1)
            .context("source result id must start at 1")?,
    );
    for line in receipt_lines.lines().rev() {
        let Ok(receipt) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if receipt
            .get("call_id")
            .and_then(Value::as_str)
            .is_some_and(|call_id| call_id.starts_with("edge-operator-inquiry-"))
            || matches!(
                receipt.get("origin").and_then(Value::as_str),
                Some("operator_harness" | "operator_inquiry_harness")
            )
        {
            continue;
        }
        if receipt.get("tool_name").and_then(Value::as_str) != Some("search_web")
            || receipt.get("status").and_then(Value::as_str) != Some("success")
        {
            continue;
        }
        let results = receipt
            .pointer("/result_summary/results")
            .and_then(Value::as_array)
            .context("latest successful search retained no results")?;
        let result = results
            .get(result_index)
            .with_context(|| format!("search result {result_id} is unavailable"))?;
        let url = result
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| {
                (url.starts_with("https://") || url.starts_with("http://"))
                    && url.chars().count() <= 2_048
            })
            .context("selected search result has no bounded public URL")?;
        return Ok(SelectedSearchResult {
            query: bounded_text(
                receipt
                    .pointer("/result_summary/query")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                300,
            ),
            title: bounded_text(
                result
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("untitled source"),
                300,
            ),
            url: url.to_string(),
            relevance_score_millis: result
                .get("relevance_score_millis")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or_default(),
            source_class: result
                .get("source_class")
                .and_then(Value::as_str)
                .unwrap_or("legacy_unclassified")
                .to_string(),
        });
    }
    bail!("no completed successful search is available")
}

pub async fn run(
    config: Arc<Config>,
    ingress_tx: mpsc::Sender<SensoryIngress>,
    action_tx: mpsc::Sender<ActionCandidate>,
    human_activity_tx: watch::Sender<u64>,
    activity_tx: broadcast::Sender<ActivityEvent>,
    autonomy_trace_registry: Arc<AutonomyTraceRegistry>,
    maintenance_work: Arc<WorkTracker>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match observe_once(
            &config,
            &ingress_tx,
            &action_tx,
            &human_activity_tx,
            &activity_tx,
            &autonomy_trace_registry,
            &maintenance_work,
        )
        .await
        {
            Ok(()) => eprintln!("Astrid IPC observer disconnected"),
            Err(error) => eprintln!("Astrid IPC observer unavailable: {error}"),
        }
        tokio::time::sleep(backoff).await;
        backoff = backoff
            .checked_mul(2)
            .unwrap_or(Duration::MAX)
            .min(Duration::from_secs(30));
        if config.astrid_socket.exists() {
            backoff = Duration::from_secs(1);
        }
    }
}

#[allow(clippy::too_many_lines)] // One authenticated IPC stream preserves event ordering.
async fn observe_once(
    config: &Config,
    ingress_tx: &mpsc::Sender<SensoryIngress>,
    action_tx: &mpsc::Sender<ActionCandidate>,
    human_activity_tx: &watch::Sender<u64>,
    activity_tx: &broadcast::Sender<ActivityEvent>,
    autonomy_trace_registry: &AutonomyTraceRegistry,
    maintenance_work: &Arc<WorkTracker>,
) -> Result<()> {
    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    maintenance_work.ipc_authenticated();
    let _ipc_epoch = IpcObservationEpoch(Arc::clone(maintenance_work));
    eprintln!("Astrid IPC observer authenticated");
    let (mut pending_web_calls, mut completed_web_calls) = load_web_call_state(config);

    loop {
        let message = read_frame(&mut stream).await?;
        if message.get("topic").and_then(Value::as_str) == Some("system.v1.maintenance_barrier") {
            match parse_maintenance_barrier(&message) {
                Ok((sequence, lease_schema, lease_kind, lease_id, lease_payload_sha256)) => {
                    maintenance_work.observe_barrier(
                        sequence,
                        &lease_schema,
                        &lease_kind,
                        &lease_id,
                        &lease_payload_sha256,
                    );
                },
                Err(error) => {
                    maintenance_work.reject_ipc_sequence();
                    eprintln!("invalid kernel maintenance barrier poisoned exactness: {error:#}");
                },
            }
            continue;
        }
        let Some(payload) = message.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("user_input") => {
                if let Some(text) = mirrored_user_text(&message) {
                    let scheduled_introspection =
                        crate::scheduled_introspection::is_scheduled_introspection_prompt(text);
                    if !is_autonomous_prompt(text) && !scheduled_introspection {
                        let _ = human_activity_tx.send(unix_millis());
                    }
                    if !scheduled_introspection {
                        let _ = activity_tx.send(ActivityEvent {
                            kind: "completed_user_input",
                            artifact_basename: None,
                            trace: message_trace(&message),
                            response_sha256: None,
                        });
                        ingress_tx
                            .send(SensoryIngress::Semantic(encode_text("user", text)))
                            .await
                            .map_err(|_| anyhow::anyhow!("reservoir ingress closed"))?;
                    }
                }
            },
            Some("agent_response") => {
                if !is_kernel_attested_react_response(&message) {
                    eprintln!(
                        "ignored agent_response with incoherent topic or unattested producer"
                    );
                    continue;
                }
                let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
                let is_final = payload
                    .get("is_final")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let response_provenance = agent_response_provenance(payload);
                if is_final && !response_provenance.may_enter_authored_paths() {
                    eprintln!(
                        "excluded non-authored terminal agent response with provenance {}",
                        response_provenance.label()
                    );
                    continue;
                }
                if is_final
                    && !terminal_agent_response_is_current(
                        config,
                        autonomy_trace_registry,
                        &message,
                        text,
                    )
                {
                    eprintln!(
                        "ignored stale autonomous terminal response: interrupted, recovered, or already consumed"
                    );
                    continue;
                }
                // Streaming transport fragments are not independent experiences.
                // Admit the completed assistant turn once; recurrence carries its
                // temporal echo after the terminal event.
                if !is_scheduled_introspection_message(&message)
                    && let Some(final_text) = final_agent_response_text(payload)
                    && let Some(experience_text) = agent_response_experience_text(final_text)
                {
                    let _ = activity_tx.send(ActivityEvent {
                        kind: "completed_assistant_turn",
                        artifact_basename: None,
                        trace: message_trace(&message),
                        response_sha256: Some(format!(
                            "{:x}",
                            Sha256::digest(experience_text.as_bytes())
                        )),
                    });
                    ingress_tx
                        .send(SensoryIngress::Semantic(encode_text(
                            "assistant",
                            experience_text,
                        )))
                        .await
                        .map_err(|_| anyhow::anyhow!("reservoir ingress closed"))?;
                }
                // Autonomous output is admitted to the Action executor only by
                // the scheduler after it has durably classified the turn as
                // genuinely authored. The IPC copy remains observational.
                if is_final
                    && !message_uses_autonomy_trace(config, autonomy_trace_registry, &message)
                {
                    let trace = message_trace(&message);
                    let tuning_authority_turn_id = response_provenance
                        .grants_exact_model_authority()
                        .then(|| trace.as_ref().and_then(|trace| trace.turn_id))
                        .flatten();
                    let tuning_authority_source = response_provenance
                        .grants_exact_model_authority()
                        .then_some("kernel_attested_exact_model_terminal_response");
                    action_tx
                        .send(ActionCandidate {
                            session_id: payload
                                .get("session_id")
                                .and_then(Value::as_str)
                                .unwrap_or("default")
                                .to_string(),
                            response: text.to_string(),
                            trace,
                            tuning_authority_turn_id,
                            tuning_authority_source,
                            maintenance_permit: Some(maintenance_work.begin_action()?),
                        })
                        .await
                        .map_err(|_| anyhow::anyhow!("action executor closed"))?;
                }
            },
            Some("tool_execute_result") => {
                if is_operator_inquiry_message(&message) {
                    continue;
                }
                let parent_response_sha256 = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .and_then(|call_id| pending_web_calls.get(call_id))
                    .and_then(|call| call.parent_response_sha256.clone())
                    .or_else(|| {
                        payload
                            .get("parent_response_sha256")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    });
                if !is_scheduled_introspection_message(&message) {
                    let _ = activity_tx.send(ActivityEvent {
                        kind: "completed_tool_result",
                        artifact_basename: None,
                        trace: message_trace(&message),
                        response_sha256: parent_response_sha256,
                    });
                    let bounded = serde_json::to_string(payload)
                        .unwrap_or_default()
                        .chars()
                        .take(8_192)
                        .collect::<String>();
                    ingress_tx
                        .send(SensoryIngress::Semantic(encode_text("tool", &bounded)))
                        .await
                        .map_err(|_| anyhow::anyhow!("reservoir ingress closed"))?;
                }
                if let Some(receipt) = completed_web_receipt(
                    &message,
                    payload,
                    &mut pending_web_calls,
                    &mut completed_web_calls,
                ) && let Err(error) = append_web_receipt(config, &receipt)
                {
                    eprintln!("edge web tool receipt persistence failed: {error}");
                }
            },
            Some("tool_execute_request") => {
                if is_operator_inquiry_message(&message) {
                    continue;
                }
                let autonomy = read_autonomy_trace_id(config);
                if let Some((call_id, call)) =
                    pending_web_call(&message, payload, autonomy.as_deref())
                    && !pending_web_calls.contains_key(&call_id)
                    && !completed_web_calls.contains(&call_id)
                {
                    let (persisted_pending, persisted_completed) = load_web_call_state(config);
                    if let Some(persisted) = persisted_pending.get(&call_id) {
                        pending_web_calls.insert(call_id, persisted.clone());
                    } else if persisted_completed.contains(&call_id) {
                        completed_web_calls.insert(call_id);
                    } else if let Err(error) =
                        append_web_receipt(config, &requested_web_receipt(&call_id, &call))
                    {
                        eprintln!("edge web request receipt persistence failed: {error}");
                    } else {
                        pending_web_calls.insert(call_id, call);
                    }
                }
            },
            _ => {},
        }
    }
}

struct IpcObservationEpoch(Arc<WorkTracker>);

impl Drop for IpcObservationEpoch {
    fn drop(&mut self) {
        self.0.ipc_disconnected();
    }
}

fn parse_maintenance_barrier(message: &Value) -> Result<(u64, String, String, String, String)> {
    let sequence = message
        .get("seq")
        .and_then(Value::as_u64)
        .filter(|sequence| *sequence > 0)
        .context("barrier sequence is absent")?;
    let producer = message
        .get("producer")
        .and_then(Value::as_object)
        .context("barrier producer attestation is absent")?;
    anyhow::ensure!(
        producer.get("schema_version").and_then(Value::as_u64) == Some(1)
            && producer.get("kind").and_then(Value::as_str) == Some("kernel_host")
            && producer.get("id").and_then(Value::as_str) == Some("maintenance_gate")
            && producer.len() == 3,
        "barrier producer attestation is not canonical"
    );
    let payload = message
        .get("payload")
        .and_then(Value::as_object)
        .context("barrier payload is absent")?;
    let allowed = [
        "type",
        "schema",
        "lease_schema",
        "lease_kind",
        "lease_id",
        "lease_payload_sha256",
        "authority",
    ];
    anyhow::ensure!(
        payload.keys().all(|key| allowed.contains(&key.as_str())) && payload.len() == allowed.len(),
        "barrier payload fields are not exact"
    );
    anyhow::ensure!(
        payload.get("type").and_then(Value::as_str) == Some("raw_json")
            && payload.get("schema").and_then(Value::as_str)
                == Some("astrid.edge.maintenance_barrier.v2")
            && payload.get("authority").and_then(Value::as_str)
                == Some("kernel_ordered_drain_barrier_not_action_authority"),
        "barrier schema or authority is invalid"
    );
    let lease_schema = payload
        .get("lease_schema")
        .and_then(Value::as_str)
        .context("barrier lease schema is absent")?;
    let lease_kind = payload
        .get("lease_kind")
        .and_then(Value::as_str)
        .context("barrier lease kind is absent")?;
    anyhow::ensure!(
        matches!(
            (lease_schema, lease_kind),
            (
                "astrid.edge_self_change.maintenance_lease.v2",
                "generation_transition"
            ) | (
                "astrid.edge_scheduled_reflection.lease.v1",
                "scheduled_reflection"
            )
        ),
        "barrier lease schema and kind are not an exact supported pair"
    );
    let lease_id = payload
        .get("lease_id")
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty() && value.len() <= 64 && !value.chars().any(char::is_control)
        })
        .context("barrier lease identity is invalid")?;
    let lease_payload_sha256 = payload
        .get("lease_payload_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_lower_hex(value, 64))
        .context("barrier lease hash is invalid")?;
    Ok((
        sequence,
        lease_schema.to_owned(),
        lease_kind.to_owned(),
        lease_id.to_owned(),
        lease_payload_sha256.to_owned(),
    ))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_kernel_attested_react_response(message: &Value) -> bool {
    if message.get("topic").and_then(Value::as_str) != Some(CANONICAL_AGENT_RESPONSE_TOPIC) {
        return false;
    }
    if message
        .get("seq")
        .and_then(Value::as_u64)
        .unwrap_or_default()
        == 0
    {
        return false;
    }
    let Some(timestamp) = message
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
    else {
        return false;
    };
    let age = Utc::now().signed_duration_since(timestamp.with_timezone(&Utc));
    if age > chrono::Duration::minutes(10) || age < chrono::Duration::seconds(-30) {
        return false;
    }
    let Some(producer) = message_producer(message) else {
        return false;
    };
    producer.kind == "wasm_capsule" && producer.id == REACT_CAPSULE_ID
}

fn message_uses_autonomy_trace(
    config: &Config,
    registry: &AutonomyTraceRegistry,
    message: &Value,
) -> bool {
    let Some(trace) = message_trace(message) else {
        return false;
    };
    match registry.observe_or_bind(&trace) {
        Ok(AutonomyTraceMatch::Registered) => true,
        Ok(AutonomyTraceMatch::RegisteredIdentityConflict) => {
            eprintln!("suppressed scheduler trace with conflicting kernel turn/session identity");
            true
        },
        Err(error) => {
            eprintln!("suppressed terminal response: autonomy trace registry failed: {error}");
            true
        },
        Ok(AutonomyTraceMatch::NotRegistered) => match autonomy_state_owns_trace(config, &trace) {
            Ok(true) => true,
            Ok(false) => match autonomy_trace_is_durable(config, &trace.trace_id.to_string()) {
                Ok(durable) => durable,
                Err(error) => {
                    eprintln!(
                        "suppressed terminal response: autonomy ledger validation failed: {error}"
                    );
                    true
                },
            },
            Err(error) => {
                eprintln!(
                    "suppressed terminal response: autonomy state validation failed: {error}"
                );
                true
            },
        },
    }
}

fn pending_web_call(
    message: &Value,
    payload: &Value,
    scheduled_trace_id: Option<&str>,
) -> Option<(String, PendingWebCall)> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if !matches!(tool_name, "search_web" | "fetch_url") {
        return None;
    }
    let call_id = payload.get("call_id").and_then(Value::as_str)?;
    let arguments =
        sanitized_web_arguments(tool_name, payload.get("arguments").unwrap_or(&Value::Null));
    let trace = message_trace(message);
    let origin = trace.as_ref().map_or("legacy_unattributed", |value| {
        if scheduled_trace_id == Some(value.trace_id.to_string().as_str()) {
            "scheduled_native_tool"
        } else {
            value
                .session_id
                .as_deref()
                .map_or("interactive_native_tool", |session| {
                    let harness_session = operator_inquiry_session_id();
                    if session == harness_session {
                        "operator_harness"
                    } else if session == scheduled_introspection_session_id() {
                        "scheduled_introspection_tool"
                    } else {
                        "interactive_native_tool"
                    }
                })
        }
    });
    Some((
        bounded_text(call_id, 160),
        PendingWebCall {
            tool_name: tool_name.to_string(),
            arguments,
            requested_at_unix_ms: unix_millis(),
            origin: origin.to_string(),
            parent_response_sha256: None,
            trace,
        },
    ))
}

fn is_operator_inquiry_message(message: &Value) -> bool {
    message_trace(message)
        .and_then(|trace| trace.session_id)
        .is_some_and(|session_id| {
            session_id == operator_inquiry_session_id()
                || session_id == operator_introspection_session_id()
        })
}

fn is_scheduled_introspection_message(message: &Value) -> bool {
    message_trace(message)
        .and_then(|trace| trace.session_id)
        .is_some_and(|session_id| session_id == scheduled_introspection_session_id())
}

fn read_autonomy_trace_id(config: &Config) -> Option<String> {
    let bytes = std::fs::read(config.workspace.join("autonomous/state.json")).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("last_trace_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Decide whether a terminal response may become bounded assistant experience.
///
/// Interactive responses are accepted and may separately reach the Action executor. A response
/// carrying the current autonomous trace is observational here and is accepted only while its
/// turn is running, or in the narrow race where the scheduler has durably completed the same
/// authored response but its Action has not been consumed yet. The scheduler alone dispatches
/// autonomous Actions after state and receipt acknowledgement. Traces already present in a
/// run/recovery ledger are old autonomous responses and are rejected.
fn terminal_agent_response_is_current(
    config: &Config,
    registry: &AutonomyTraceRegistry,
    message: &Value,
    text: &str,
) -> bool {
    let Some(trace) = message_trace(message).filter(IpcTraceContextV1::is_supported) else {
        return true;
    };
    match registry.observe_or_bind(&trace) {
        Ok(AutonomyTraceMatch::Registered) => {},
        Ok(AutonomyTraceMatch::RegisteredIdentityConflict) | Err(_) => return false,
        Ok(AutonomyTraceMatch::NotRegistered) => {
            if autonomy_state_owns_trace(config, &trace).unwrap_or(true)
                || autonomy_trace_is_durable(config, &trace.trace_id.to_string()).unwrap_or(true)
            {
                return false;
            }
            return true;
        },
    }
    let Ok(Some(state)) = read_autonomy_state(config) else {
        return false;
    };
    if !state_value_owns_trace(&state, &trace) {
        return false;
    }
    if state.get("last_status").and_then(Value::as_str) == Some("running") {
        return true;
    }
    let response_sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
    state.get("last_status").and_then(Value::as_str) == Some("authored_completed")
        && state.get("last_response_sha256").and_then(Value::as_str)
            == Some(response_sha256.as_str())
        && state
            .get("last_action_response_sha256")
            .and_then(Value::as_str)
            != Some(response_sha256.as_str())
}

fn read_autonomy_state(config: &Config) -> Result<Option<Value>> {
    let path = config.workspace.join("autonomous/state.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() {
        bail!(
            "autonomy state is not a regular non-symlink file: {}",
            path.display()
        );
    }
    let bytes = fs::read(&path)?;
    Ok(Some(
        serde_json::from_slice(&bytes).context("parse autonomy state for IPC authority")?,
    ))
}

fn state_value_owns_trace(state: &Value, trace: &IpcTraceContextV1) -> bool {
    let trace_id = trace.trace_id.to_string();
    if state.get("last_trace_id").and_then(Value::as_str) != Some(trace_id.as_str()) {
        return false;
    }
    let Some(stored) = state
        .get("last_trace")
        .and_then(|value| serde_json::from_value::<IpcTraceContextV1>(value.clone()).ok())
    else {
        // Transitional v2/v3 state recorded only the exact root trace ID.
        return true;
    };
    stored.is_supported()
        && stored.trace_id == trace.trace_id
        && stored.session_id == trace.session_id
        && stored.chain_id == trace.chain_id
}

fn autonomy_state_owns_trace(config: &Config, trace: &IpcTraceContextV1) -> Result<bool> {
    Ok(read_autonomy_state(config)?
        .as_ref()
        .is_some_and(|state| state_value_owns_trace(state, trace)))
}

fn autonomy_trace_is_durable(config: &Config, trace_id: &str) -> Result<bool> {
    for relative in ["autonomous/runs.jsonl", "autonomous/recoveries.jsonl"] {
        let path = config.workspace.join(relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            bail!(
                "autonomy ledger is not a regular non-symlink file: {}",
                path.display()
            );
        }
        for line in BufReader::new(fs::File::open(&path)?).lines() {
            let value = serde_json::from_str::<Value>(&line?)
                .with_context(|| format!("parse autonomy ledger {}", path.display()))?;
            if value.pointer("/trace/trace_id").and_then(Value::as_str) == Some(trace_id) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn completed_web_receipt(
    message: &Value,
    payload: &Value,
    pending: &mut HashMap<String, PendingWebCall>,
    completed: &mut HashSet<String>,
) -> Option<WebToolReceipt> {
    let call_id = payload.get("call_id").and_then(Value::as_str)?;
    if completed.contains(call_id) {
        return None;
    }
    let source_topic = message
        .get("topic")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let pending_call = pending.remove(call_id);
    let tool_name = pending_call
        .as_ref()
        .map(|call| call.tool_name.as_str())
        .or_else(|| web_tool_name_from_result_topic(source_topic))?
        .to_string();
    if !matches!(tool_name.as_str(), "search_web" | "fetch_url") {
        return None;
    }
    let result = payload.get("result")?;
    let content = result
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let is_error = result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let completed_at_unix_ms = unix_millis();
    let trace = message_trace(message).or_else(|| {
        pending_call
            .as_ref()
            .and_then(|call| call.trace.as_ref().map(IpcTraceContextV1::child))
    });
    let requested_at_unix_ms = pending_call
        .as_ref()
        .map_or(completed_at_unix_ms, |call| call.requested_at_unix_ms);
    let origin = pending_call.as_ref().map_or_else(
        || "legacy_unattributed".to_string(),
        |call| call.origin.clone(),
    );
    let parent_response_sha256 = pending_call
        .as_ref()
        .and_then(|call| call.parent_response_sha256.clone());
    let arguments = pending_call
        .as_ref()
        .map_or(Value::Null, |call| call.arguments.clone());
    completed.insert(call_id.to_string());
    Some(WebToolReceipt {
        schema: WEB_RECEIPT_SCHEMA.to_string(),
        phase: "completed".to_string(),
        recorded_at_unix_ms: completed_at_unix_ms,
        requested_at_unix_ms,
        completed_at_unix_ms: Some(completed_at_unix_ms),
        latency_ms: Some(completed_at_unix_ms.saturating_sub(requested_at_unix_ms)),
        call_id: bounded_text(call_id, 160),
        tool_name: tool_name.clone(),
        arguments,
        status: if is_error { "error" } else { "success" }.to_string(),
        result_summary: Some(summarize_web_result(&tool_name, content, is_error)),
        result_sha256: Some(format!("{:x}", Sha256::digest(content.as_bytes()))),
        source_topic: bounded_text(source_topic, 300),
        origin,
        parent_response_sha256,
        trace,
        authority: "observed_read_only_tool_result_not_model_authorship".to_string(),
    })
}

fn completed_direct_web_receipt(
    call_id: &str,
    pending: &PendingWebCall,
    tool_name: &str,
    content: &str,
    is_error: bool,
) -> WebToolReceipt {
    let completed_at_unix_ms = unix_millis();
    WebToolReceipt {
        schema: WEB_RECEIPT_SCHEMA.to_string(),
        phase: "completed".to_string(),
        recorded_at_unix_ms: completed_at_unix_ms,
        requested_at_unix_ms: pending.requested_at_unix_ms,
        completed_at_unix_ms: Some(completed_at_unix_ms),
        latency_ms: Some(completed_at_unix_ms.saturating_sub(pending.requested_at_unix_ms)),
        call_id: bounded_text(call_id, 160),
        tool_name: tool_name.to_string(),
        arguments: pending.arguments.clone(),
        status: if is_error { "error" } else { "success" }.to_string(),
        result_summary: Some(summarize_web_result(tool_name, content, is_error)),
        result_sha256: Some(format!("{:x}", Sha256::digest(content.as_bytes()))),
        source_topic: format!("immutable.web_broker.v1.{tool_name}"),
        origin: pending.origin.clone(),
        parent_response_sha256: pending.parent_response_sha256.clone(),
        trace: pending.trace.as_ref().map(IpcTraceContextV1::child),
        authority: "immutable_read_only_web_broker_result_not_model_authorship".to_string(),
    }
}

fn requested_web_receipt(call_id: &str, call: &PendingWebCall) -> WebToolReceipt {
    WebToolReceipt {
        schema: WEB_RECEIPT_SCHEMA.to_string(),
        phase: "requested".to_string(),
        recorded_at_unix_ms: call.requested_at_unix_ms,
        requested_at_unix_ms: call.requested_at_unix_ms,
        completed_at_unix_ms: None,
        latency_ms: None,
        call_id: bounded_text(call_id, 160),
        tool_name: call.tool_name.clone(),
        arguments: call.arguments.clone(),
        status: "requested".to_string(),
        result_summary: None,
        result_sha256: None,
        source_topic: format!("tool.v1.execute.{}", call.tool_name),
        origin: call.origin.clone(),
        parent_response_sha256: call.parent_response_sha256.clone(),
        trace: call.trace.clone(),
        authority: "observed_read_only_tool_request_not_model_authorship".to_string(),
    }
}

fn load_web_call_state(config: &Config) -> (HashMap<String, PendingWebCall>, HashSet<String>) {
    let path = config.workspace.join("web/receipts.jsonl");
    let Ok(file) = std::fs::File::open(&path) else {
        return (HashMap::new(), HashSet::new());
    };
    let mut pending = HashMap::new();
    let mut completed = HashSet::new();
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("schema").and_then(Value::as_str) != Some(WEB_RECEIPT_SCHEMA) {
            // v1 was completion-only. It remains readable, but cannot be
            // causally joined to a request without an exact identifier.
            if let Some(call_id) = value.get("call_id").and_then(Value::as_str) {
                completed.insert(call_id.to_string());
            }
            continue;
        }
        let Some(call_id) = value
            .get("call_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        match value.get("phase").and_then(Value::as_str) {
            Some("requested") => {
                let Ok(receipt) = serde_json::from_value::<WebToolReceipt>(value) else {
                    continue;
                };
                pending.insert(
                    call_id,
                    PendingWebCall {
                        tool_name: receipt.tool_name,
                        arguments: receipt.arguments,
                        requested_at_unix_ms: receipt.requested_at_unix_ms,
                        origin: receipt.origin,
                        parent_response_sha256: receipt.parent_response_sha256,
                        trace: receipt.trace,
                    },
                );
            },
            Some("completed") => {
                pending.remove(&call_id);
                completed.insert(call_id);
            },
            _ => {},
        }
    }
    (pending, completed)
}

fn web_tool_name_from_result_topic(topic: &str) -> Option<&str> {
    topic
        .strip_prefix("tool.v1.execute.")
        .and_then(|value| value.strip_suffix(".result"))
        .filter(|name| matches!(*name, "search_web" | "fetch_url"))
}

fn sanitized_web_arguments(tool_name: &str, arguments: &Value) -> Value {
    let mut sanitized = serde_json::Map::new();
    let names: &[&str] = match tool_name {
        "search_web" => &["query", "count"],
        "fetch_url" => &["url", "method", "max_chars"],
        _ => &[],
    };
    for name in names {
        if let Some(value) = arguments.get(*name) {
            sanitized.insert((*name).to_string(), value.clone());
        }
    }
    Value::Object(sanitized)
}

fn contextualized_research_query(config: &Config, original: &str) -> String {
    let mut terms = query_terms(original);
    if let Ok(value) = std::fs::read(config.workspace.join("autonomous/thread_state.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .ok_or(())
    {
        for field in ["focus", "question", "hypothesis"] {
            if let Some(text) = value.get(field).and_then(Value::as_str) {
                terms.extend(query_terms(text));
            }
        }
        if let Some(values) = value.get("hypotheses").and_then(Value::as_array) {
            for text in values.iter().filter_map(Value::as_str).take(2) {
                terms.extend(query_terms(text));
            }
        }
    }
    terms.extend(
        ["technical", "paper", "documentation", "architecture"]
            .into_iter()
            .map(str::to_string),
    );
    terms.sort();
    terms.dedup();
    let context = terms.into_iter().take(12).collect::<Vec<_>>().join(" ");
    bounded_text(&format!("{original} {context}"), 300)
}

fn search_has_useful_result(content: &str, original_query: &str) -> bool {
    serde_json::from_str::<Value>(content)
        .ok()
        .is_some_and(|value| {
            ranked_search_results(&value, original_query)
                .iter()
                .any(|result| {
                    result
                        .get("relevance_score_millis")
                        .and_then(Value::as_u64)
                        .is_some_and(|score| score >= 120)
                })
        })
}

fn ranked_search_results(value: &Value, original_query: &str) -> Vec<Value> {
    let query = if original_query.trim().is_empty() {
        value
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
    } else {
        original_query
    };
    let query_tokens = query_terms(query);
    let mut results = value
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(10)
        .map(|result| {
            let title = result.get("title").and_then(Value::as_str).unwrap_or("");
            let snippet = result
                .get("snippet")
                .or_else(|| result.get("description"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let url = result.get("url").and_then(Value::as_str).unwrap_or("");
            let candidate_terms = query_terms(&format!("{title} {snippet}"));
            let overlap = query_tokens
                .iter()
                .filter(|term| candidate_terms.contains(term))
                .count();
            let denominator = query_tokens.len().max(1);
            let base = overlap
                .saturating_mul(1_000)
                .checked_div(denominator)
                .unwrap_or_default();
            let source_class = classify_source(url);
            let bonus = match source_class {
                "primary_or_scholarly" => 100,
                "official_documentation" => 60,
                _ => 0,
            };
            let score = base.saturating_add(bonus).min(1_000);
            json!({
                "title": bounded_text(title, 300),
                "url": bounded_text(url, 2_048),
                "snippet": bounded_text(snippet, 400),
                "source_class": source_class,
                "relevance_score_millis": score,
            })
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .get("relevance_score_millis")
            .and_then(Value::as_u64)
            .cmp(&left.get("relevance_score_millis").and_then(Value::as_u64))
            .then_with(|| {
                left.get("url")
                    .and_then(Value::as_str)
                    .cmp(&right.get("url").and_then(Value::as_str))
            })
    });
    results.truncate(3);
    results
}

fn query_terms(value: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "about", "after", "before", "could", "does", "from", "have", "into", "that", "their",
        "these", "this", "what", "when", "where", "which", "with", "would",
    ];
    let mut terms = value
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 4 && !STOP.contains(&term.as_str()))
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

fn classify_source(url: &str) -> &'static str {
    let url = url.to_ascii_lowercase();
    if [
        "arxiv.org",
        "doi.org",
        "acm.org",
        "ieee.org",
        "springer.com",
        "nature.com",
        "sciencedirect.com",
        "pubmed.ncbi.nlm.nih.gov",
    ]
    .iter()
    .any(|domain| url.contains(domain))
    {
        "primary_or_scholarly"
    } else if [
        "docs.rs",
        "rust-lang.org",
        "github.com",
        "ollama.com",
        "huggingface.co",
        "developer.",
        "/docs/",
    ]
    .iter()
    .any(|domain| url.contains(domain))
    {
        "official_documentation"
    } else {
        "general_web"
    }
}

fn summarize_web_result(tool_name: &str, content: &str, is_error: bool) -> Value {
    if is_error {
        return json!({"error": bounded_text(content, 500)});
    }
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return json!({"unparsed_excerpt": bounded_text(content, 500)});
    };
    match tool_name {
        "search_web" => {
            let query = value.get("query").and_then(Value::as_str).unwrap_or("");
            let results = ranked_search_results(&value, query);
            let useful = results.iter().any(|result| {
                result
                    .get("relevance_score_millis")
                    .and_then(Value::as_u64)
                    .is_some_and(|score| score >= 120)
            });
            json!({
                "schema": value.get("schema").and_then(Value::as_str).unwrap_or(""),
                "query": value.get("query").and_then(Value::as_str).unwrap_or(""),
                "provider": value.get("provider").and_then(Value::as_str).unwrap_or(""),
                "result_count": value.get("result_count").and_then(Value::as_u64).unwrap_or(0),
                "results": results,
                "relevance_status": if useful { "useful_candidates" } else { "no_useful_evidence" },
            })
        },
        "fetch_url" => json!({
            "url": value.get("url").and_then(Value::as_str).unwrap_or(""),
            "status": value.get("status").and_then(Value::as_u64).unwrap_or(0),
            "original_body_bytes": value
                .get("original_body_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            "truncated": value.get("truncated").and_then(Value::as_bool).unwrap_or(false),
        }),
        _ => Value::Null,
    }
}

fn summarize_introspection_result(value: &Value, content: &str, is_error: bool) -> Value {
    if is_error {
        return json!({
            "error_class": "bounded_tool_error",
            "error_chars": content.chars().count().min(500)
        });
    }
    let matches = value
        .get("matches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(8)
        .map(|entry| {
            json!({
                "kind": entry.get("kind").and_then(Value::as_str).unwrap_or(""),
                "basename": entry.get("basename").and_then(Value::as_str).unwrap_or("")
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": value.get("schema").and_then(Value::as_str).unwrap_or(""),
        "question": value
            .get("question")
            .or_else(|| value.get("query"))
            .and_then(Value::as_str)
            .map(|query| bounded_text(query, 160))
            .unwrap_or_default(),
        "files_considered": value
            .get("files_considered")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "match_count": value
            .get("match_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "matches": matches,
    })
}

fn summarize_spectral_result(
    tool_name: &str,
    value: &Value,
    content: &str,
    is_error: bool,
) -> Value {
    if is_error {
        return json!({
            "error_class": "bounded_spectral_tool_error",
            "error_chars": content.chars().count().min(500),
        });
    }
    let common = json!({
        "schema": value.get("schema").and_then(Value::as_str).unwrap_or(""),
        "source": value.get("source").and_then(Value::as_str).unwrap_or(""),
        "authority": value.get("authority").and_then(Value::as_str).unwrap_or(""),
        "causality": value.get("causality").and_then(Value::as_str).unwrap_or(""),
    });
    match tool_name {
        "read_spectral_now" => json!({
            "common": common,
            "recorded_at_unix_ms": value
                .get("recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            "state_sha256": bounded_text(
                value.get("state_sha256").and_then(Value::as_str).unwrap_or(""),
                64,
            ),
            "substrate_kind": bounded_text(
                value.pointer("/substrate/kind").and_then(Value::as_str).unwrap_or(""),
                80,
            ),
            "metric_names": spectral_metric_names(value),
            "metric_values": sanitized_spectral_metric_values(
                value.get("metrics").unwrap_or(&Value::Null),
            ),
        }),
        "read_spectral_window" => json!({
            "common": common,
            "window_minutes": value.get("window_minutes").and_then(Value::as_u64).unwrap_or(0),
            "count": value.get("count").and_then(Value::as_u64).unwrap_or(0),
            "first_recorded_at_unix_ms": value
                .get("first_recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            "last_recorded_at_unix_ms": value
                .get("last_recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            "projection_sha256": bounded_text(
                value.get("projection_sha256").and_then(Value::as_str).unwrap_or(""),
                64,
            ),
            "trailing_partial": value
                .get("trailing_partial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "metric_names": spectral_metric_names(value),
            "metric_summaries": sanitized_spectral_metric_summaries(
                value.get("metrics").unwrap_or(&Value::Null),
            ),
        }),
        "correlate_spectral_activity" => json!({
            "common": common,
            "exact_filter": sanitized_spectral_exact_filter(
                value.get("exact_filter").unwrap_or(&Value::Null),
            ),
            "count": value.get("count").and_then(Value::as_u64).unwrap_or(0).min(20),
            "correlated_activity": sanitized_spectral_correlations(
                value.get("matches").unwrap_or(&Value::Null),
            ),
            "attribution_rule": bounded_text(
                value.get("attribution_rule").and_then(Value::as_str).unwrap_or(""),
                160,
            ),
        }),
        _ => json!({"common": common}),
    }
}

fn is_allowed_spectral_metric(name: &str) -> bool {
    matches!(
        name,
        "fill_pct"
            | "effective_dimensionality"
            | "spectral_entropy"
            | "lambda1_share"
            | "head_share"
            | "shoulder_share"
            | "tail_share"
            | "density_gradient"
            | "mode_turnover"
    )
}

fn rounded_spectral_number(value: &Value) -> Option<Value> {
    let value = value.as_f64().filter(|value| value.is_finite())?;
    Some(json!((value * 1_000_000.0).round() / 1_000_000.0))
}

fn sanitized_spectral_metric_values(value: &Value) -> Value {
    let Some(metrics) = value.as_object() else {
        return json!({});
    };
    Value::Object(
        metrics
            .iter()
            .filter(|(name, _)| is_allowed_spectral_metric(name))
            .filter_map(|(name, value)| {
                rounded_spectral_number(value).map(|value| (name.clone(), value))
            })
            .take(9)
            .collect(),
    )
}

fn sanitized_spectral_metric_summaries(value: &Value) -> Value {
    let Some(metrics) = value.as_object() else {
        return json!({});
    };
    let summaries = metrics
        .iter()
        .filter(|(name, _)| is_allowed_spectral_metric(name))
        .filter_map(|(name, summary)| {
            let count = summary.get("count").and_then(Value::as_u64).unwrap_or(0);
            let bounded = json!({
                "count": count.min(1_440),
                "min": summary.get("min").and_then(rounded_spectral_number),
                "mean": summary.get("mean").and_then(rounded_spectral_number),
                "max": summary.get("max").and_then(rounded_spectral_number),
            });
            (count > 0).then(|| (name.clone(), bounded))
        })
        .take(9)
        .collect();
    Value::Object(summaries)
}

fn sanitized_spectral_correlations(value: &Value) -> Value {
    let Some(matches) = value.as_array() else {
        return json!([]);
    };
    Value::Array(
        matches
            .iter()
            .take(4)
            .map(|entry| {
                let matched_fields = entry
                    .get("matched_fields")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|field| {
                        matches!(
                            *field,
                            "trace_id"
                                | "session_id"
                                | "chain_id"
                                | "response_sha256"
                                | "parent_response_sha256"
                        )
                    })
                    .take(4)
                    .map(|field| Value::String(field.to_string()))
                    .collect::<Vec<_>>();
                json!({
                    "recorded_at_unix_ms": entry
                        .get("recorded_at_unix_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    "activity_kind": bounded_text(
                        entry.get("activity_kind").and_then(Value::as_str).unwrap_or(""),
                        40,
                    ),
                    "event_kind": bounded_text(
                        entry.get("event_kind").and_then(Value::as_str).unwrap_or(""),
                        64,
                    ),
                    "status": bounded_text(
                        entry.get("status").and_then(Value::as_str).unwrap_or(""),
                        64,
                    ),
                    "matched_fields": matched_fields,
                    "metric_values": sanitized_spectral_metric_values(
                        entry.pointer("/spectral/metrics").unwrap_or(&Value::Null),
                    ),
                })
            })
            .collect(),
    )
}

fn sanitized_spectral_exact_filter(value: &Value) -> Value {
    let mut sanitized = serde_json::Map::new();
    for name in ["trace_id", "session_id", "chain_id", "response_sha256"] {
        if let Some(value) = value.get(name).and_then(Value::as_str) {
            let maximum = if name == "response_sha256" { 64 } else { 128 };
            sanitized.insert(name.to_string(), json!(bounded_text(value, maximum)));
        }
    }
    Value::Object(sanitized)
}

fn spectral_metric_names(value: &Value) -> Vec<String> {
    let metrics = value
        .get("metrics")
        .or_else(|| value.get("summaries"))
        .and_then(Value::as_object);
    metrics
        .into_iter()
        .flat_map(serde_json::Map::keys)
        .filter(|name| is_allowed_spectral_metric(name))
        .take(9)
        .cloned()
        .collect()
}

fn append_web_receipt(config: &Config, receipt: &WebToolReceipt) -> Result<()> {
    let path = config.workspace.join("web/receipts.jsonl");
    append_web_receipt_path(&path, receipt)
}

fn append_web_receipt_path(path: &Path, receipt: &WebToolReceipt) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("open web receipt ledger {}", path.display()))?;
    serde_json::to_writer(&mut file, receipt)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn append_introspection_receipt(config: &Config, receipt: &IntrospectionReceipt) -> Result<()> {
    let path = config.workspace.join("introspection/receipts.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("open introspection receipt ledger {}", path.display()))?;
    serde_json::to_writer(&mut file, receipt)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn append_spectral_receipt(config: &Config, receipt: &SpectralReceipt) -> Result<()> {
    let directory = config.workspace.join("spectral");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create spectral receipt directory {}", directory.display()))?;
    let path = directory.join("receipts.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("open spectral receipt ledger {}", path.display()))?;
    serde_json::to_writer(&mut file, receipt)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn bounded_text(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn unix_millis() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn mirrored_user_text(message: &Value) -> Option<&str> {
    if message.get("topic").and_then(Value::as_str) != Some("sensory.v1.user_input") {
        return None;
    }
    let payload = message.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("user_input") {
        return None;
    }
    payload.get("text").and_then(Value::as_str)
}

fn final_agent_response_text(payload: &Value) -> Option<&str> {
    if payload.get("is_final").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    payload
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentResponseProvenance {
    LegacyUnspecified,
    ModelAuthored,
    ModelAuthoredWithLocalSafeFallback,
    ModelAuthoredWithLocalFormatRepair,
    ExecutorTerminalError,
    Invalid,
}

impl AgentResponseProvenance {
    const fn may_enter_authored_paths(self) -> bool {
        matches!(
            self,
            Self::ModelAuthored
                | Self::ModelAuthoredWithLocalSafeFallback
                | Self::ModelAuthoredWithLocalFormatRepair
        )
    }

    const fn grants_exact_model_authority(self) -> bool {
        matches!(self, Self::ModelAuthored)
    }

    const fn label(self) -> &'static str {
        match self {
            Self::LegacyUnspecified => "legacy_unspecified",
            Self::ModelAuthored => "model_authored",
            Self::ModelAuthoredWithLocalSafeFallback => "model_authored_with_local_safe_fallback",
            Self::ModelAuthoredWithLocalFormatRepair => "model_authored_with_local_format_repair",
            Self::ExecutorTerminalError => "executor_terminal_error",
            Self::Invalid => "invalid",
        }
    }
}

fn agent_response_provenance(payload: &Value) -> AgentResponseProvenance {
    match payload.get("response_provenance") {
        None | Some(Value::Null) => AgentResponseProvenance::LegacyUnspecified,
        Some(Value::String(value)) if value == "model_authored" => {
            AgentResponseProvenance::ModelAuthored
        },
        Some(Value::String(value)) if value == "model_authored_with_local_safe_fallback" => {
            AgentResponseProvenance::ModelAuthoredWithLocalSafeFallback
        },
        Some(Value::String(value)) if value == "model_authored_with_local_format_repair" => {
            AgentResponseProvenance::ModelAuthoredWithLocalFormatRepair
        },
        Some(Value::String(value)) if value == "executor_terminal_error" => {
            AgentResponseProvenance::ExecutorTerminalError
        },
        Some(_) => AgentResponseProvenance::Invalid,
    }
}

/// Return only the portion of a terminal response that may become assistant
/// experience. Transport recovery is never experience; when the executor
/// appended a safe fallback to a non-empty model prefix, only that authored
/// prefix is admitted.
fn agent_response_experience_text(text: &str) -> Option<&str> {
    if transport_recovery_reason(text).is_some() {
        return None;
    }
    match model_authored_prefix_before_safe_fallback(text) {
        Some(prefix) => (!prefix.trim().is_empty()).then_some(prefix),
        None => Some(text),
    }
}

/// Execute one scheduled model turn over the already-authenticated native
/// daemon socket without spawning the mutable CLI binary.
pub(crate) async fn execute_direct_headless_turn(
    config: &Config,
    prompt: &str,
    session_name: &str,
    requested_trace: &IpcTraceContextV1,
    idle_timeout: Duration,
) -> Result<DirectHeadlessTurn> {
    let session_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, session_name.as_bytes());
    let session_id_text = session_id.to_string();
    if !requested_trace.is_supported()
        || requested_trace.session_id.as_deref() != Some(session_id_text.as_str())
    {
        bail!("direct headless session does not match the scheduler trace");
    }

    let mut stream = UnixStream::connect(&config.astrid_socket)
        .await
        .with_context(|| format!("connect {}", config.astrid_socket.display()))?;
    authenticate(&mut stream, &config.astrid_token).await?;
    write_frame(
        &mut stream,
        &direct_user_input(prompt, session_id, requested_trace),
    )
    .await?;

    let mut response = String::new();
    let mut canonical_trace: Option<IpcTraceContextV1> = None;
    loop {
        let message = tokio::time::timeout(idle_timeout, read_frame(&mut stream))
            .await
            .context("direct headless IPC idle deadline expired")??;
        if message_trace(&message).is_none_or(|trace| trace.trace_id != requested_trace.trace_id) {
            continue;
        }
        let Some(payload) = message.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("approval_required") => {
                deny_direct_headless_approval(&mut stream, payload, session_id).await?;
            },
            Some("agent_response") => {
                let Some(attested) = direct_canonical_response_trace(
                    &message,
                    payload,
                    &session_id_text,
                    requested_trace,
                ) else {
                    continue;
                };
                if canonical_trace
                    .as_ref()
                    .is_some_and(|prior| prior.turn_id != attested.turn_id)
                {
                    bail!("kernel-attested direct response changed turn identity");
                }
                canonical_trace = Some(attested.clone());
                if let Some(text) = payload.get("text").and_then(Value::as_str) {
                    if response.len().saturating_add(text.len())
                        > DIRECT_HEADLESS_MAX_RESPONSE_BYTES
                    {
                        bail!("direct headless response exceeded its immutable bound");
                    }
                    response.push_str(text);
                }
                if payload.get("is_final").and_then(Value::as_bool) != Some(true) {
                    continue;
                }
                if response.trim().is_empty() {
                    bail!("direct headless response was empty");
                }
                let provenance = payload
                    .get("response_provenance")
                    .and_then(Value::as_str)
                    .filter(|value| {
                        matches!(
                            *value,
                            "model_authored"
                                | "model_authored_with_local_safe_fallback"
                                | "model_authored_with_local_format_repair"
                        )
                    })
                    .context("direct headless terminal provenance is absent or non-authored")?;
                let provider_metrics_receipt = direct_provider_metrics_receipt(&message, &attested);
                let _ = write_frame(
                    &mut stream,
                    &direct_disconnect(session_id, Some(requested_trace)),
                )
                .await;
                return Ok(DirectHeadlessTurn {
                    response,
                    canonical_trace: attested,
                    response_provenance: provenance.to_string(),
                    provider_metrics_receipt,
                });
            },
            _ => {},
        }
    }
}

fn direct_user_input(prompt: &str, session_id: Uuid, trace: &IpcTraceContextV1) -> Value {
    direct_message(
        "user.v1.prompt",
        &json!({
            "type": "user_input",
            "text": prompt,
            "session_id": session_id.to_string(),
            "context": null
        }),
        session_id,
        Some(trace),
    )
}

fn direct_disconnect(session_id: Uuid, trace: Option<&IpcTraceContextV1>) -> Value {
    direct_message(
        "client.v1.disconnect",
        &json!({"type": "disconnect", "reason": "edge-direct-headless"}),
        session_id,
        trace,
    )
}

fn direct_message(
    topic: &str,
    payload: &Value,
    source_id: Uuid,
    trace: Option<&IpcTraceContextV1>,
) -> Value {
    json!({
        "topic": topic,
        "payload": payload,
        "signature": null,
        "source_id": source_id,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "seq": 0,
        "trace": trace,
        "local_provider_metrics": null
    })
}

async fn deny_direct_headless_approval(
    stream: &mut UnixStream,
    payload: &Value,
    session_id: Uuid,
) -> Result<()> {
    let request_id = payload
        .get("request_id")
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 256
                && !value.chars().any(char::is_control)
                && !value.contains('/')
                && !value.contains('\\')
        })
        .context("direct headless approval request identifier is invalid")?;
    write_frame(
        stream,
        &direct_message(
            &format!("astrid.v1.approval.response.{request_id}"),
            &json!({
                "type": "approval_response",
                "request_id": request_id,
                "decision": "deny",
                "reason": "CPU-edge direct headless mode",
                "boundary_id": null
            }),
            session_id,
            None,
        ),
    )
    .await
}

fn direct_canonical_response_trace(
    message: &Value,
    payload: &Value,
    session_id: &str,
    requested: &IpcTraceContextV1,
) -> Option<IpcTraceContextV1> {
    if !is_kernel_attested_react_response(message)
        || payload.get("type").and_then(Value::as_str) != Some("agent_response")
        || payload.get("session_id").and_then(Value::as_str) != Some(session_id)
    {
        return None;
    }
    let trace = message_trace(message)?;
    (trace.trace_id == requested.trace_id
        && trace.turn_id.is_some()
        && trace.session_id.as_deref() == Some(session_id)
        && trace.chain_id == requested.chain_id)
        .then_some(trace)
}

fn direct_provider_metrics_receipt(
    message: &Value,
    canonical_trace: &IpcTraceContextV1,
) -> Option<Value> {
    let metrics = message.get("local_provider_metrics")?.as_object()?;
    if metrics.keys().any(|key| {
        !matches!(
            key.as_str(),
            "schema_version"
                | "producer"
                | "request_count"
                | "successful_header_count"
                | "requests"
        )
    }) {
        return None;
    }
    if metrics.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let producer = metrics.get("producer")?.as_object()?;
    if producer
        .keys()
        .any(|key| !matches!(key.as_str(), "schema_version" | "kind" | "id"))
    {
        return None;
    }
    if producer.get("schema_version").and_then(Value::as_u64) != Some(1)
        || producer.get("kind").and_then(Value::as_str) != Some("kernel_host")
        || producer.get("id").and_then(Value::as_str) != Some("wasm_http_stream")
    {
        return None;
    }
    let request_count = metrics.get("request_count")?.as_u64()?;
    let successful_header_count = metrics.get("successful_header_count")?.as_u64()?;
    let raw_requests = metrics.get("requests")?.as_array()?;
    if request_count == 0
        || request_count > 16
        || successful_header_count > request_count
        || usize::try_from(request_count).ok()? != raw_requests.len()
    {
        return None;
    }
    let requests = sanitized_direct_provider_requests(raw_requests, successful_header_count)?;
    let single = if request_count == 1 && successful_header_count == 1 {
        let request = requests.first()?.as_object()?;
        Some((
            request.get("request_id")?.clone(),
            request.get("request_header_latency_ms")?.clone(),
        ))
    } else {
        None
    };
    let mut receipt = json!({
        "schema_version": 1,
        "trace": canonical_trace,
        "producer": producer,
        "request_count": request_count,
        "successful_header_count": successful_header_count,
        "requests": requests
    });
    if let Some((request_id, latency)) = single {
        receipt["request_id"] = request_id;
        receipt["request_header_latency_ms"] = latency;
    }
    Some(receipt)
}

fn sanitized_direct_provider_requests(
    raw_requests: &[Value],
    successful_header_count: u64,
) -> Option<Vec<Value>> {
    let mut requests = Vec::with_capacity(raw_requests.len());
    let mut attempt_ids = HashSet::new();
    let mut observed_successful_headers = 0_u64;
    for raw_request in raw_requests {
        let request = raw_request.as_object()?;
        if request.keys().any(|key| {
            !matches!(
                key.as_str(),
                "attempt_id" | "request_id" | "outcome" | "request_header_latency_ms"
            )
        }) {
            return None;
        }
        let attempt_id = Uuid::parse_str(request.get("attempt_id")?.as_str()?).ok()?;
        let request_id = Uuid::parse_str(request.get("request_id")?.as_str()?).ok()?;
        if attempt_id.is_nil() || request_id.is_nil() || !attempt_ids.insert(attempt_id) {
            return None;
        }
        let outcome = request.get("outcome")?.as_str()?;
        let latency = request
            .get("request_header_latency_ms")
            .and_then(Value::as_u64);
        match (outcome, latency) {
            ("successful_headers", Some(_)) => {
                observed_successful_headers = observed_successful_headers.checked_add(1)?;
            },
            (
                "non_success_status" | "unknown_peer" | "non_loopback_peer" | "timeout"
                | "transport_error" | "cancelled",
                None,
            ) => {},
            _ => return None,
        }
        let mut sanitized = json!({
            "attempt_id": attempt_id,
            "request_id": request_id,
            "outcome": outcome
        });
        if let Some(latency) = latency {
            sanitized["request_header_latency_ms"] = json!(latency);
        }
        requests.push(sanitized);
    }
    if observed_successful_headers != successful_header_count {
        return None;
    }
    Some(requests)
}

async fn authenticate(stream: &mut UnixStream, token_path: &Path) -> Result<()> {
    let token = tokio::fs::read_to_string(token_path)
        .await
        .with_context(|| format!("read token {}", token_path.display()))?;
    let request = json!({
        "token": token.trim(),
        "protocol_version": 1,
        "client_version": env!("CARGO_PKG_VERSION"),
    });
    write_frame(stream, &request).await?;
    let response = read_frame(stream).await?;
    if response.get("status").and_then(Value::as_str) != Some("ok") {
        bail!(
            "daemon rejected handshake: {}",
            response
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown reason")
        );
    }
    Ok(())
}

async fn read_frame(stream: &mut UnixStream) -> Result<Value> {
    let mut length_bytes = [0_u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length > MAX_FRAME_SIZE {
        bail!("IPC frame exceeds {MAX_FRAME_SIZE} bytes");
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload).context("decode IPC frame")
}

async fn write_frame(stream: &mut UnixStream, value: &Value) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    let length = u32::try_from(payload.len()).context("IPC frame too large")?;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AgentResponseProvenance, SelectedSearchResult, SpectralQuery,
        agent_response_experience_text, agent_response_provenance, completed_web_receipt,
        direct_canonical_response_trace, direct_provider_metrics_receipt, direct_user_input,
        final_agent_response_text, introspection_search_request, is_kernel_attested_react_response,
        is_operator_inquiry_message, latest_search_result, load_web_call_state,
        message_uses_autonomy_trace, mirrored_user_text, operator_inquiry_session_id,
        parse_maintenance_barrier, pending_web_call, ranked_search_results, requested_web_receipt,
        research_search_request, source_fetch_request, spectral_tool_arguments,
        spectral_tool_request, summarize_introspection_result, summarize_spectral_result,
        terminal_agent_response_is_current, verified_capsule_tool_result,
        web_tool_name_from_result_topic,
    };
    use crate::{
        config::Config,
        trace::{AutonomyTraceRegistry, IpcTraceContextV1},
    };
    use clap::Parser as _;
    use serde_json::{Value, json};
    use sha2::Digest as _;
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    #[test]
    fn direct_headless_user_input_binds_the_exact_scheduler_session_and_trace() {
        let session_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"edge-autonomy-session-g7");
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            session_id.to_string(),
            Some("chain-2".to_string()),
        );
        let message = direct_user_input("bounded prompt", session_id, &trace);

        assert_eq!(message["topic"], "user.v1.prompt");
        assert_eq!(message["source_id"], session_id.to_string());
        assert_eq!(message["payload"]["type"], "user_input");
        assert_eq!(message["payload"]["text"], "bounded prompt");
        assert_eq!(message["payload"]["session_id"], session_id.to_string());
        assert_eq!(message["trace"]["trace_id"], trace.trace_id.to_string());
        assert_eq!(message["trace"]["session_id"], session_id.to_string());
        assert_eq!(message["trace"]["chain_id"], "chain-2");
        assert!(message.get("producer").is_none());
        assert!(message.get("principal").is_none());
    }

    #[test]
    fn direct_headless_accepts_only_canonical_react_output_with_exact_identity() {
        let session_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"edge-autonomy-session-g8");
        let requested = IpcTraceContextV1::root(
            Uuid::new_v4(),
            session_id.to_string(),
            Some("chain-3".to_string()),
        );
        let mut canonical_trace = requested.child();
        canonical_trace.turn_id = Some(Uuid::new_v4());
        let canonical = json!({
            "topic": "agent.v1.response",
            "seq": 42,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "producer": {
                "schema_version": 1,
                "kind": "wasm_capsule",
                "id": "astrid-capsule-react"
            },
            "trace": canonical_trace,
            "payload": {
                "type": "agent_response",
                "session_id": session_id,
                "text": "NEXT: LISTEN",
                "is_final": true,
                "response_provenance": "model_authored"
            }
        });
        assert_eq!(
            direct_canonical_response_trace(
                &canonical,
                &canonical["payload"],
                &session_id.to_string(),
                &requested,
            ),
            serde_json::from_value(canonical["trace"].clone()).ok()
        );

        for (field, replacement) in [
            ("trace_id", json!(Uuid::new_v4())),
            ("session_id", json!(Uuid::new_v4().to_string())),
            ("chain_id", json!("foreign-chain")),
        ] {
            let mut wrong = canonical.clone();
            wrong["trace"][field] = replacement;
            assert!(
                direct_canonical_response_trace(
                    &wrong,
                    &wrong["payload"],
                    &session_id.to_string(),
                    &requested,
                )
                .is_none(),
                "accepted mismatched {field}"
            );
        }

        let mut forged = canonical.clone();
        forged["producer"]["kind"] = json!("native_socket_client");
        assert!(
            direct_canonical_response_trace(
                &forged,
                &forged["payload"],
                &session_id.to_string(),
                &requested,
            )
            .is_none()
        );
    }

    #[test]
    fn direct_provider_metrics_are_body_free_and_bound_to_the_canonical_trace() {
        let trace =
            IpcTraceContextV1::root(Uuid::new_v4(), "edge-autonomy-session".to_string(), None);
        let attempt_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let message = json!({
            "local_provider_metrics": {
                "schema_version": 1,
                "producer": {
                    "schema_version": 1,
                    "kind": "kernel_host",
                    "id": "wasm_http_stream"
                },
                "request_count": 1,
                "successful_header_count": 1,
                "requests": [{
                    "attempt_id": attempt_id,
                    "request_id": request_id,
                    "outcome": "successful_headers",
                    "request_header_latency_ms": 17,
                }]
            }
        });
        let receipt = direct_provider_metrics_receipt(&message, &trace).unwrap();
        assert_eq!(receipt["trace"]["trace_id"], trace.trace_id.to_string());
        assert_eq!(receipt["request_id"], request_id.to_string());
        assert_eq!(receipt["request_header_latency_ms"], 17);

        let mut body_bearing = message.clone();
        body_bearing["local_provider_metrics"]["requests"][0]["response_body"] =
            json!("must-not-be-copied");
        assert!(direct_provider_metrics_receipt(&body_bearing, &trace).is_none());

        let mut malformed = message;
        malformed["local_provider_metrics"]["producer"]["kind"] = json!("mutable_runtime");
        assert!(direct_provider_metrics_receipt(&malformed, &trace).is_none());
    }

    #[test]
    fn maintenance_barrier_requires_exact_kernel_attestation() {
        let barrier = json!({
            "topic": "system.v1.maintenance_barrier",
            "seq": 42,
            "producer": {
                "schema_version": 1,
                "kind": "kernel_host",
                "id": "maintenance_gate"
            },
            "payload": {
                "type": "raw_json",
                "schema": "astrid.edge.maintenance_barrier.v2",
                "lease_schema": "astrid.edge_scheduled_reflection.lease.v1",
                "lease_kind": "scheduled_reflection",
                "lease_id": "lease-example",
                "lease_payload_sha256": "a".repeat(64),
                "authority": "kernel_ordered_drain_barrier_not_action_authority"
            }
        });
        assert_eq!(
            parse_maintenance_barrier(&barrier).unwrap(),
            (
                42,
                "astrid.edge_scheduled_reflection.lease.v1".to_string(),
                "scheduled_reflection".to_string(),
                "lease-example".to_string(),
                "a".repeat(64)
            )
        );
        let mut forged = barrier;
        forged["producer"]["kind"] = json!("native_socket_client");
        assert!(parse_maintenance_barrier(&forged).is_err());
    }

    #[test]
    fn research_search_request_is_bounded_and_read_only() {
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "test-session".to_string(),
            Some("chain-1".to_string()),
        );
        let request = research_search_request("edge-research-1", &"x".repeat(300), Some(&trace));
        assert_eq!(request["topic"], "tool.v1.execute.search_web");
        assert_eq!(request["payload"]["type"], "tool_execute_request");
        assert_eq!(request["payload"]["tool_name"], "search_web");
        assert_eq!(request["payload"]["arguments"]["count"], 5);
        assert_eq!(
            request["payload"]["arguments"]["query"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            300
        );
        assert!(request["payload"]["arguments"].get("url").is_none());
        assert!(request["payload"]["arguments"].get("method").is_none());
        assert_eq!(request["trace"]["trace_id"], trace.trace_id.to_string());
    }

    #[test]
    fn introspection_request_and_receipt_summary_are_bounded_and_body_free() {
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "test-session".to_string(),
            Some("chain-1".to_string()),
        );
        let request =
            introspection_search_request("edge-introspection-1", &"x".repeat(160), Some(&trace));
        assert_eq!(request["topic"], "tool.v1.execute.inspect_owned_question");
        assert_eq!(request["payload"]["tool_name"], "inspect_owned_question");
        assert_eq!(request["payload"]["arguments"]["limit"], 8);
        assert_eq!(
            request["payload"]["arguments"]["question"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            160
        );
        assert!(request["payload"]["arguments"].get("path").is_none());

        let body = json!({
            "schema": "astrid_edge_owned_question_inspection_v1",
            "question": "heat",
            "files_considered": 3,
            "match_count": 1,
            "matches": [{
                "kind": "journal",
                "basename": "journal_1.md",
                "excerpt": "private returned body that must not enter receipts"
            }]
        });
        let summary = summarize_introspection_result(&body, &body.to_string(), false);
        assert_eq!(summary["match_count"], 1);
        assert_eq!(summary["matches"][0]["basename"], "journal_1.md");
        assert!(summary.to_string().find("private returned body").is_none());
    }

    #[test]
    fn spectral_requests_are_model_hidden_bounded_and_exactly_correlated() {
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "spectral-session".to_string(),
            Some("spectral-chain".to_string()),
        );
        let response_sha256 = "a".repeat(64);
        let (tool, arguments) = spectral_tool_arguments(
            SpectralQuery::Correlate { limit: 12 },
            &trace,
            &response_sha256,
        )
        .unwrap();
        assert_eq!(tool, "correlate_spectral_activity");
        assert_eq!(arguments["trace_id"], trace.trace_id.to_string());
        assert_eq!(arguments["session_id"], "spectral-session");
        assert_eq!(arguments["chain_id"], "spectral-chain");
        assert_eq!(arguments["response_sha256"], response_sha256);
        assert_eq!(arguments["limit"], 12);

        let request = spectral_tool_request("spectral-call", tool, &arguments, Some(&trace));
        assert_eq!(
            request["topic"],
            "tool.v1.execute.correlate_spectral_activity"
        );
        assert_eq!(request["payload"]["tool_name"], tool);
        assert!(request["payload"]["arguments"].get("path").is_none());
        assert!(request["payload"]["arguments"].get("control").is_none());

        assert!(
            spectral_tool_arguments(
                SpectralQuery::Window { minutes: 30 },
                &trace,
                &response_sha256,
            )
            .is_err()
        );
        assert!(
            spectral_tool_arguments(
                SpectralQuery::Correlate { limit: 0 },
                &trace,
                &response_sha256,
            )
            .is_err()
        );
    }

    #[test]
    fn spectral_receipt_summary_excludes_matches_and_returned_bodies() {
        let result = json!({
            "schema": "astrid_edge_spectral_correlation_result_v1",
            "source": "recent_rollups_projection",
            "authority": "machine_read_only",
            "causality": "not_established",
            "exact_filter": {
                "trace_id": Uuid::new_v4(),
                "response_sha256": "b".repeat(64),
                "unexpected": "must-not-persist"
            },
            "count": 2,
            "matches": [{
                "private": "returned correlation body must not enter the receipt",
                "recorded_at_unix_ms": 42,
                "activity_kind": "sovereign_action_outcome",
                "event_kind": "trial_completed",
                "status": "success",
                "matched_fields": ["trace_id", "timestamp_proximity"],
                "spectral": {
                    "metrics": {"spectral_entropy": 0.912_345_67, "secret": 9.0}
                }
            }],
            "attribution_rule": "explicit identifiers only"
        });
        let summary = summarize_spectral_result(
            "correlate_spectral_activity",
            &result,
            &result.to_string(),
            false,
        );
        let rendered = summary.to_string();
        assert_eq!(summary["count"], 2);
        assert_eq!(summary["causality"], Value::Null);
        assert!(rendered.contains("not_established"));
        assert!(!rendered.contains("returned correlation body"));
        assert!(!rendered.contains("must-not-persist"));
        assert!(!rendered.contains("secret"));
        assert_eq!(
            summary["correlated_activity"][0]["metric_values"]["spectral_entropy"],
            json!(0.912_346)
        );
        assert_eq!(
            summary["correlated_activity"][0]["matched_fields"],
            json!(["trace_id"])
        );
        assert!(summary["exact_filter"].get("unexpected").is_none());
    }

    #[test]
    fn spectral_receipt_summary_retains_bounded_numeric_evidence() {
        let now = json!({
            "schema": "astrid_edge_spectral_now_result_v1",
            "metrics": {
                "fill_pct": 68.123_456_7,
                "spectral_entropy": 0.934_567_89,
                "lambda1_share": 0.21,
                "unapproved_metric": 99.0
            },
            "authority": "machine_read_only",
            "causality": "none_claimed"
        });
        let now_summary =
            summarize_spectral_result("read_spectral_now", &now, &now.to_string(), false);
        assert_eq!(now_summary["metric_values"]["fill_pct"], json!(68.123_457));
        assert_eq!(
            now_summary["metric_values"]["spectral_entropy"],
            json!(0.934_568)
        );
        assert!(
            now_summary["metric_values"]
                .get("unapproved_metric")
                .is_none()
        );

        let window = json!({
            "schema": "astrid_edge_spectral_window_result_v1",
            "count": 60,
            "metrics": {
                "tail_share": {"count": 60, "min": 0.1, "mean": 0.2, "max": 0.3},
                "mode_turnover": {"count": 0, "min": null, "mean": null, "max": null}
            },
            "authority": "machine_read_only",
            "causality": "none_claimed"
        });
        let window_summary =
            summarize_spectral_result("read_spectral_window", &window, &window.to_string(), false);
        assert_eq!(
            window_summary["metric_summaries"]["tail_share"]["mean"],
            json!(0.2)
        );
        assert!(
            window_summary["metric_summaries"]
                .get("mode_turnover")
                .is_none()
        );
    }

    #[test]
    fn source_fetch_selects_only_a_retained_result_from_the_latest_successful_search() {
        let receipts = [
            r#"{"tool_name":"search_web","status":"success","result_summary":{"query":"older","results":[{"title":"Old","url":"https://old.example"}]}}"#,
            r#"{"tool_name":"fetch_url","status":"success","result_summary":{"url":"https://ignored.example"}}"#,
            r#"{"tool_name":"search_web","status":"success","result_summary":{"query":"latest","results":[{"title":"First","url":"https://one.example"},{"title":"Second","url":"https://two.example"}]}}"#,
            r#"{"call_id":"edge-operator-inquiry-search-1","origin":"operator_inquiry_harness","tool_name":"search_web","status":"success","result_summary":{"query":"operator-only","results":[{"title":"Private harness","url":"https://operator.example"}]}}"#,
        ]
        .join("\n");
        assert_eq!(
            latest_search_result(&receipts, 2).unwrap(),
            SelectedSearchResult {
                query: "latest".to_string(),
                title: "Second".to_string(),
                url: "https://two.example".to_string(),
                relevance_score_millis: 0,
                source_class: "legacy_unclassified".to_string(),
            }
        );
        assert!(latest_search_result(&receipts, 0).is_err());
        assert!(latest_search_result(&receipts, 3).is_err());

        let request = source_fetch_request("edge-read-source-1", "https://two.example", None);
        assert_eq!(request["topic"], "tool.v1.execute.fetch_url");
        assert_eq!(request["payload"]["tool_name"], "fetch_url");
        assert_eq!(request["payload"]["arguments"]["method"], "GET");
        assert_eq!(request["payload"]["arguments"]["max_chars"], 64 * 1_024);
        assert!(request["payload"]["arguments"].get("headers").is_none());
    }

    #[test]
    fn technical_relevance_ranking_rejects_burst_frequency_genetics_drift() {
        let value = json!({
            "query": "reservoir burst frequency scheduler cadence",
            "results": [
                {
                    "title": "Genetic burst frequency in cell expression",
                    "snippet": "A genetics assay of transcription bursts",
                    "url": "https://example.org/genetics"
                },
                {
                    "title": "Echo state reservoir scheduler cadence",
                    "snippet": "Technical analysis of reservoir sampling and periodic scheduler aliases",
                    "url": "https://arxiv.org/abs/1234.5678"
                }
            ]
        });
        let ranked = ranked_search_results(&value, "reservoir burst frequency scheduler cadence");
        assert_eq!(ranked[0]["source_class"], "primary_or_scholarly");
        assert!(ranked[0]["title"].as_str().unwrap().contains("reservoir"));
        assert!(
            ranked[0]["relevance_score_millis"].as_u64().unwrap()
                > ranked[1]["relevance_score_millis"].as_u64().unwrap()
        );
    }

    #[test]
    fn only_the_explicit_sensory_mirror_drives_user_input() {
        let mirrored = json!({
            "topic": "sensory.v1.user_input",
            "payload": {"type": "user_input", "text": "hello"}
        });
        assert_eq!(mirrored_user_text(&mirrored), Some("hello"));

        let original = json!({
            "topic": "user.v1.prompt",
            "payload": {"type": "user_input", "text": "hello"}
        });
        assert_eq!(mirrored_user_text(&original), None);
    }

    #[test]
    fn assistant_stream_fragments_are_not_terminal_responses() {
        let fragment = json!({
            "type": "agent_response",
            "text": "partial",
            "is_final": false
        });
        let terminal = json!({
            "type": "agent_response",
            "text": "complete",
            "is_final": true
        });

        assert_eq!(final_agent_response_text(&fragment), None);
        assert_eq!(final_agent_response_text(&terminal), Some("complete"));
        assert_eq!(
            final_agent_response_text(&json!({
                "type": "agent_response",
                "text": "",
                "is_final": true
            })),
            None
        );
    }

    #[test]
    fn interactive_executor_errors_cannot_enter_experience_or_actions() {
        let executor_error = json!({
            "type": "agent_response",
            "text": "LLM error: unavailable",
            "is_final": true,
            "response_provenance": "executor_terminal_error"
        });
        let provenance = agent_response_provenance(&executor_error);
        assert_eq!(provenance, AgentResponseProvenance::ExecutorTerminalError);
        assert!(!provenance.may_enter_authored_paths());
        assert!(!provenance.grants_exact_model_authority());

        let exact_model = json!({"response_provenance": "model_authored"});
        let provenance = agent_response_provenance(&exact_model);
        assert!(provenance.may_enter_authored_paths());
        assert!(provenance.grants_exact_model_authority());

        let formatting_repair = json!({
            "response_provenance": "model_authored_with_local_format_repair"
        });
        let provenance = agent_response_provenance(&formatting_repair);
        assert!(provenance.may_enter_authored_paths());
        assert!(!provenance.grants_exact_model_authority());

        let malformed = json!({"response_provenance": 7});
        assert!(!agent_response_provenance(&malformed).may_enter_authored_paths());
        assert!(
            !agent_response_provenance(&json!({})).may_enter_authored_paths(),
            "legacy responses remain decodable history but cannot enter new authored paths"
        );
        assert!(!agent_response_provenance(&json!({})).grants_exact_model_authority());
    }

    #[test]
    fn only_canonical_kernel_attested_react_output_is_a_response() {
        let canonical = json!({
            "topic": "agent.v1.response",
            "seq": 42,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "producer": {
                "schema_version": 1,
                "kind": "wasm_capsule",
                "id": "astrid-capsule-react"
            },
            "payload": {"type": "agent_response", "text": "NEXT: LISTEN", "is_final": true}
        });
        assert!(is_kernel_attested_react_response(&canonical));

        let mut smuggled = canonical.clone();
        smuggled["topic"] = json!("tool.v1.execute.search_web.result");
        assert!(!is_kernel_attested_react_response(&smuggled));

        let mut socket_spoof = canonical.clone();
        socket_spoof["producer"]["kind"] = json!("native_socket_client");
        assert!(!is_kernel_attested_react_response(&socket_spoof));

        let mut foreign_capsule = canonical;
        foreign_capsule["producer"]["id"] = json!("astrid-capsule-edge-spectral");
        assert!(!is_kernel_attested_react_response(&foreign_capsule));
    }

    #[test]
    fn private_tool_results_require_exact_capsule_topic_and_child_lineage() {
        let request_trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "private-tool-session".to_string(),
            Some("private-tool-chain".to_string()),
        );
        let result_trace = request_trace.child();
        let exact = json!({
            "topic": "tool.v1.execute.inspect_owned_question.result",
            "producer": {
                "schema_version": 1,
                "kind": "wasm_capsule",
                "id": "astrid-capsule-edge-introspector"
            },
            "trace": result_trace,
            "payload": {
                "type": "tool_execute_result",
                "call_id": "predictable-call-id",
                "result": {"content": "{}", "is_error": false}
            }
        });
        assert!(
            verified_capsule_tool_result(
                &exact,
                "predictable-call-id",
                "inspect_owned_question",
                "astrid-capsule-edge-introspector",
                &request_trace,
            )
            .is_some()
        );

        let mut wrong_topic = exact.clone();
        wrong_topic["topic"] = json!("tool.v1.execute.read_owned_artifact.result");
        assert!(
            verified_capsule_tool_result(
                &wrong_topic,
                "predictable-call-id",
                "inspect_owned_question",
                "astrid-capsule-edge-introspector",
                &request_trace,
            )
            .is_none()
        );

        let mut wrong_producer = exact.clone();
        wrong_producer["producer"]["id"] = json!("astrid-capsule-fs");
        assert!(
            verified_capsule_tool_result(
                &wrong_producer,
                "predictable-call-id",
                "inspect_owned_question",
                "astrid-capsule-edge-introspector",
                &request_trace,
            )
            .is_none()
        );

        let mut socket_spoof = exact.clone();
        socket_spoof["producer"]["kind"] = json!("native_socket_client");
        assert!(
            verified_capsule_tool_result(
                &socket_spoof,
                "predictable-call-id",
                "inspect_owned_question",
                "astrid-capsule-edge-introspector",
                &request_trace,
            )
            .is_none()
        );

        for field in ["trace_id", "session_id", "chain_id", "parent_span_id"] {
            let mut wrong_lineage = exact.clone();
            wrong_lineage["trace"][field] = match field {
                "trace_id" | "parent_span_id" => json!(Uuid::new_v4()),
                "session_id" => json!("foreign-session"),
                "chain_id" => json!("foreign-chain"),
                _ => unreachable!("bounded test fixture"),
            };
            assert!(
                verified_capsule_tool_result(
                    &wrong_lineage,
                    "predictable-call-id",
                    "inspect_owned_question",
                    "astrid-capsule-edge-introspector",
                    &request_trace,
                )
                .is_none(),
                "accepted wrong {field}"
            );
        }

        assert!(
            verified_capsule_tool_result(
                &exact,
                "different-call-id",
                "inspect_owned_question",
                "astrid-capsule-edge-introspector",
                &request_trace,
            )
            .is_none()
        );
    }

    #[test]
    fn executor_fallback_never_enters_assistant_experience() {
        let marker = "[Local contract repair: no valid final action was emitted; defaulting safely to LISTEN.]";
        let only_fallback = format!("{marker}\nNEXT: LISTEN");
        assert_eq!(agent_response_experience_text(&only_fallback), None);

        let authored_prefix = format!("A bounded model observation.\n\n{marker}\nNEXT: LISTEN");
        assert_eq!(
            agent_response_experience_text(&authored_prefix),
            Some("A bounded model observation.")
        );

        let recovery = format!(
            "Request timed out (Streaming phase exceeded 600s limit)\n\n{marker}\nNEXT: LISTEN"
        );
        assert_eq!(agent_response_experience_text(&recovery), None);

        let authored_listen = "Astrid chose stillness.\nNEXT: LISTEN";
        assert_eq!(
            agent_response_experience_text(authored_listen),
            Some(authored_listen)
        );
    }

    #[test]
    fn interrupted_or_consumed_autonomy_response_cannot_reach_experience_or_actions() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-terminal-authority-{}",
            super::unix_millis()
        ));
        std::fs::create_dir_all(workspace.join("autonomous")).unwrap();
        let config = Config::parse_from([
            "test",
            "--workspace",
            workspace.to_str().unwrap_or_default(),
        ]);
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "autonomous-session".to_string(),
            Some("chain-1".to_string()),
        );
        let message = json!({"trace": trace});
        let text = "A bounded response.\n\nNEXT: LISTEN";
        let response_sha256 = format!("{:x}", sha2::Sha256::digest(text.as_bytes()));
        let registry = AutonomyTraceRegistry::default();
        registry.register(&trace).unwrap();

        std::fs::write(
            workspace.join("autonomous/state.json"),
            serde_json::to_vec(&json!({
                "last_trace_id": trace.trace_id,
                "last_status": "running"
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(terminal_agent_response_is_current(
            &config, &registry, &message, text
        ));

        std::fs::write(
            workspace.join("autonomous/state.json"),
            serde_json::to_vec(&json!({
                "last_trace_id": trace.trace_id,
                "last_status": "interrupted_by_restart"
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(!terminal_agent_response_is_current(
            &config, &registry, &message, text
        ));

        std::fs::write(
            workspace.join("autonomous/state.json"),
            serde_json::to_vec(&json!({
                "last_trace_id": trace.trace_id,
                "last_status": "authored_completed",
                "last_response_sha256": response_sha256
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(terminal_agent_response_is_current(
            &config, &registry, &message, text
        ));

        std::fs::write(
            workspace.join("autonomous/state.json"),
            serde_json::to_vec(&json!({
                "last_trace_id": trace.trace_id,
                "last_status": "authored_completed",
                "last_response_sha256": response_sha256,
                "last_action_response_sha256": response_sha256
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(!terminal_agent_response_is_current(
            &config, &registry, &message, text
        ));

        let old_trace =
            IpcTraceContextV1::root(Uuid::new_v4(), "old-autonomous-session".to_string(), None);
        std::fs::write(
            workspace.join("autonomous/recoveries.jsonl"),
            format!("{}\n", json!({"status": "interrupted", "trace": old_trace})),
        )
        .unwrap();
        let old_message = json!({"trace": old_trace});
        assert!(!terminal_agent_response_is_current(
            &config,
            &registry,
            &old_message,
            text
        ));

        let interactive =
            IpcTraceContextV1::root(Uuid::new_v4(), "interactive-session".to_string(), None);
        assert!(terminal_agent_response_is_current(
            &config,
            &registry,
            &json!({"trace": interactive}),
            text
        ));
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn durable_running_trace_stays_scheduler_owned_across_observer_restart() {
        let workspace = std::env::temp_dir().join(format!(
            "astrid-edge-restarted-terminal-authority-{}",
            super::unix_millis()
        ));
        std::fs::create_dir_all(workspace.join("autonomous")).unwrap();
        let config = Config::parse_from([
            "test",
            "--workspace",
            workspace.to_str().unwrap_or_default(),
        ]);
        let trace = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "autonomous-session".to_string(),
            Some("chain-1".to_string()),
        );
        let message = json!({"trace": trace});
        std::fs::write(
            workspace.join("autonomous/state.json"),
            serde_json::to_vec(&json!({
                "last_trace_id": trace.trace_id,
                "last_trace": trace,
                "last_status": "running"
            }))
            .unwrap(),
        )
        .unwrap();
        let restarted_registry = AutonomyTraceRegistry::default();

        assert!(!terminal_agent_response_is_current(
            &config,
            &restarted_registry,
            &message,
            "late response\nNEXT: LISTEN"
        ));
        assert!(message_uses_autonomy_trace(
            &config,
            &restarted_registry,
            &message
        ));
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn public_web_receipts_are_bounded_and_drop_request_headers_and_page_bodies() {
        let request = json!({
            "type": "tool_execute_request",
            "call_id": "web-1",
            "tool_name": "fetch_url",
            "arguments": {
                "url": "https://example.com",
                "method": "GET",
                "headers": {"Authorization": "must-not-be-retained"},
                "max_chars": 16000
            }
        });
        let envelope = json!({"trace": {
            "schema_version": 1,
            "trace_id": Uuid::new_v4(),
            "span_id": Uuid::new_v4()
        }});
        let (call_id, call) = pending_web_call(&envelope, &request, None).unwrap();
        assert!(call.arguments.get("headers").is_none());
        let mut pending = HashMap::from([(call_id, call)]);
        let mut completed = HashSet::new();
        let message = json!({
            "topic": "tool.v1.execute.fetch_url.result",
            "payload": {
                "type": "tool_execute_result",
                "call_id": "web-1",
                "result": {
                    "content": "{\"url\":\"https://example.com\",\"status\":200,\
                                \"body\":\"page text\",\"original_body_bytes\":9,\
                                \"truncated\":false}",
                    "is_error": false
                }
            }
        });
        let receipt =
            completed_web_receipt(&message, &message["payload"], &mut pending, &mut completed)
                .unwrap();
        assert_eq!(receipt.tool_name, "fetch_url");
        assert_eq!(receipt.status, "success");
        assert_eq!(receipt.result_summary.as_ref().unwrap()["status"], 200);
        assert!(
            receipt
                .result_summary
                .as_ref()
                .unwrap()
                .get("body")
                .is_none()
        );
        assert!(
            receipt
                .result_sha256
                .as_ref()
                .is_some_and(|hash| !hash.is_empty())
        );
        assert!(completed.contains("web-1"));
    }

    #[test]
    fn observer_restart_reloads_only_exact_unmatched_request_ids() {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-web-reload-{}", super::unix_millis()));
        std::fs::create_dir_all(workspace.join("web")).unwrap();
        let config = Config::parse_from([
            "test",
            "--workspace",
            workspace.to_str().unwrap_or_default(),
        ]);
        let trace =
            IpcTraceContextV1::root(Uuid::new_v4(), "test-session".to_string(), Some("c".into()));
        let call = super::PendingWebCall {
            tool_name: "search_web".to_string(),
            arguments: json!({"query": "bounded"}),
            requested_at_unix_ms: 10,
            origin: "react_model_tool".to_string(),
            parent_response_sha256: None,
            trace: Some(trace),
        };
        super::append_web_receipt(&config, &requested_web_receipt("pending", &call)).unwrap();
        let mut completion = requested_web_receipt("done", &call);
        completion.phase = "completed".to_string();
        completion.status = "success".to_string();
        completion.completed_at_unix_ms = Some(20);
        super::append_web_receipt(&config, &completion).unwrap();

        let (pending, completed) = load_web_call_state(&config);
        assert!(pending.contains_key("pending"));
        assert!(!pending.contains_key("done"));
        assert!(completed.contains("done"));
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn web_tool_name_can_be_recovered_from_the_result_topic() {
        assert_eq!(
            web_tool_name_from_result_topic("tool.v1.execute.search_web.result"),
            Some("search_web")
        );
        assert_eq!(
            web_tool_name_from_result_topic("tool.v1.execute.run_shell.result"),
            None
        );
    }

    #[test]
    fn duplicate_and_out_of_order_web_results_never_manufacture_parentage() {
        let message = json!({
            "topic": "tool.v1.execute.search_web.result",
            "payload": {
                "type": "tool_execute_result",
                "call_id": "out-of-order",
                "result": {"content": "provider failed", "is_error": true}
            }
        });
        let mut pending = HashMap::new();
        let mut completed = HashSet::new();
        let receipt =
            completed_web_receipt(&message, &message["payload"], &mut pending, &mut completed)
                .unwrap();
        assert_eq!(receipt.status, "error");
        assert_eq!(receipt.origin, "legacy_unattributed");
        assert!(receipt.parent_response_sha256.is_none());
        assert!(receipt.trace.is_none());
        assert!(
            completed_web_receipt(&message, &message["payload"], &mut pending, &mut completed,)
                .is_none()
        );
    }

    #[test]
    fn native_tool_origin_comes_from_exact_session_identity() {
        let request = json!({
            "type": "tool_execute_request",
            "call_id": "web-1",
            "tool_name": "search_web",
            "arguments": {"query": "bounded"}
        });
        for (session, expected, scheduled) in [
            ("edge-autonomous-g1", "scheduled_native_tool", true),
            (
                "edge-operator-inquiry-harness-v1",
                "operator_harness",
                false,
            ),
            ("human-session", "interactive_native_tool", false),
        ] {
            let session_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, session.as_bytes()).to_string();
            let trace = IpcTraceContextV1::root(Uuid::new_v4(), session_id, None);
            let trace_id = trace.trace_id.to_string();
            let envelope = json!({
                "trace": trace
            });
            let (_, call) =
                pending_web_call(&envelope, &request, scheduled.then_some(trace_id.as_str()))
                    .unwrap();
            assert_eq!(call.origin, expected);
            assert!(call.parent_response_sha256.is_none());
        }

        let operator_trace =
            IpcTraceContextV1::root(Uuid::new_v4(), operator_inquiry_session_id(), None);
        assert!(is_operator_inquiry_message(
            &json!({"trace": operator_trace})
        ));
    }
}
