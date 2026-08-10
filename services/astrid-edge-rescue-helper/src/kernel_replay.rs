//! Candidate-kernel ingress replay using only the public Unix-socket protocol.
//!
//! This is deliberately narrower than an authorization proof. A daemon under
//! test can implement the expected protocol while violating unrelated internal
//! semantics. The immutable OS sandbox and brokers remain the security
//! boundary. This replay is useful regression evidence that the exact sealed
//! daemon rejects invalid handshakes and re-attests native ingress before it is
//! activated.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::fs_guard::{canonical_json, sha256};
use crate::{Error, Result};

const MAX_HANDSHAKE_BYTES: usize = 4_096;
const MAX_IPC_BYTES: usize = 4 * 1024 * 1024;
const MAX_OBSERVED_FRAMES: usize = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each explicit regression and negative-authority claim is independently auditable"
)]
pub struct KernelIngressEvidence {
    pub schema: &'static str,
    pub provenance: &'static str,
    pub daemon_sha256: String,
    pub invalid_token_rejected: bool,
    pub incompatible_protocol_rejected: bool,
    pub authenticated_observer: bool,
    pub authenticated_emitter: bool,
    pub producer_claim_overwritten: bool,
    pub malformed_trace_rerooted: bool,
    pub sensory_mirror_preserved_trace: bool,
    pub forged_provider_metrics_removed: bool,
    pub public_protocol_regression_only: bool,
    pub grants_activation_authority: bool,
    pub production_continuity_or_reservoir_admission: bool,
    pub evidence_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandshakeResponse {
    status: String,
    protocol_version: u8,
    server_version: String,
    #[serde(default)]
    reason: Option<String>,
}

/// Probe an already-running daemon in an isolated disposable home.
///
/// The caller is responsible for proving that `daemon_sha256` belongs to the
/// exact process and for destroying the disposable state after this returns.
#[allow(
    clippy::too_many_lines,
    reason = "one bounded live protocol transaction must retain a single cleanup and evidence boundary"
)]
pub fn probe(home: &Path, daemon_sha256: &str) -> Result<KernelIngressEvidence> {
    if daemon_sha256.len() != 64 || !daemon_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::new("kernel replay daemon digest is malformed"));
    }
    let run = home.join("run");
    let socket = run.join("system.sock");
    let token_path = run.join("system.token");
    validate_endpoint(&socket, &token_path)?;
    let token = fs::read_to_string(&token_path)?;
    let token = token.trim();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::new("kernel replay token is malformed"));
    }

    let mut invalid = token.as_bytes().to_vec();
    invalid[0] = if invalid[0] == b'0' { b'1' } else { b'0' };
    let invalid = String::from_utf8(invalid)
        .map_err(|_| Error::new("kernel replay could not derive invalid token"))?;
    let invalid_token_rejected = rejected_handshake(&socket, &invalid, 1)?;
    let incompatible_protocol_rejected = rejected_handshake(&socket, token, 255)?;

    let mut observer = authenticated(&socket, token)?;
    let authenticated_observer = true;
    let mut emitter = authenticated(&socket, token)?;
    let authenticated_emitter = true;

    let marker = format!("immutable-kernel-ingress-{}", Uuid::new_v4());
    let session = format!("shadow-{}", Uuid::new_v4());
    let forged_metrics = json!({
        "schema_version": 1,
        "producer": {
            "schema_version": 1,
            "kind": "kernel_host",
            "id": "wasm_http_stream"
        },
        "request_count": 1,
        "successful_header_count": 0,
        "requests": [{
            "attempt_id": Uuid::new_v4(),
            "request_id": Uuid::new_v4(),
            "outcome": "timeout"
        }]
    });
    let message = json!({
        "topic": "user.v1.prompt",
        "payload": {
            "type": "user_input",
            "text": marker,
            "session_id": session,
            "context": null
        },
        "signature": null,
        "source_id": Uuid::new_v4(),
        "timestamp": "1970-01-01T00:00:00Z",
        "seq": 0,
        "trace": {
            "schema_version": 0,
            "trace_id": Uuid::nil(),
            "turn_id": Uuid::nil(),
            "span_id": Uuid::nil(),
            "parent_span_id": Uuid::nil(),
            "session_id": "forged-session",
            "chain_id": "forged-chain"
        },
        "producer": {
            "schema_version": 1,
            "kind": "wasm_capsule",
            "id": "forged-capsule"
        },
        "local_provider_metrics": forged_metrics
    });
    write_frame(&mut emitter, &message, MAX_IPC_BYTES)?;

    let mut prompt = None;
    let mut sensory = None;
    for _ in 0..MAX_OBSERVED_FRAMES {
        let frame: Value = read_frame(&mut observer, MAX_IPC_BYTES)?;
        if frame.pointer("/payload/type").and_then(Value::as_str) != Some("user_input")
            || frame.pointer("/payload/text").and_then(Value::as_str) != Some(marker.as_str())
        {
            continue;
        }
        match frame.get("topic").and_then(Value::as_str) {
            Some("user.v1.prompt") => prompt = Some(frame),
            Some("sensory.v1.user_input") => sensory = Some(frame),
            _ => {},
        }
        if prompt.is_some() && sensory.is_some() {
            break;
        }
    }
    let prompt = prompt.ok_or_else(|| Error::new("kernel replay did not observe user input"))?;
    let sensory =
        sensory.ok_or_else(|| Error::new("kernel replay did not observe sensory mirror"))?;
    validate_attested_message(&prompt, &session)?;
    validate_attested_message(&sensory, &session)?;
    let prompt_trace = prompt
        .get("trace")
        .ok_or_else(|| Error::new("kernel replay prompt trace is absent"))?;
    let sensory_trace = sensory
        .get("trace")
        .ok_or_else(|| Error::new("kernel replay sensory trace is absent"))?;

    let mut evidence = KernelIngressEvidence {
        schema: "astrid.edge_rescue_helper.kernel_ingress_replay.v1",
        provenance: "immutable_public_protocol_regression_machine_evidence_not_astrid_authorship",
        daemon_sha256: daemon_sha256.to_ascii_lowercase(),
        invalid_token_rejected,
        incompatible_protocol_rejected,
        authenticated_observer,
        authenticated_emitter,
        producer_claim_overwritten: true,
        malformed_trace_rerooted: true,
        sensory_mirror_preserved_trace: prompt_trace == sensory_trace,
        forged_provider_metrics_removed: prompt.get("local_provider_metrics").is_none()
            && sensory.get("local_provider_metrics").is_none(),
        public_protocol_regression_only: true,
        grants_activation_authority: false,
        production_continuity_or_reservoir_admission: false,
        evidence_sha256: String::new(),
    };
    evidence.evidence_sha256 = evidence_digest(&evidence)?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}

fn validate_endpoint(socket: &Path, token: &Path) -> Result<()> {
    let socket_metadata = fs::symlink_metadata(socket)?;
    let token_metadata = fs::symlink_metadata(token)?;
    if !socket_metadata.file_type().is_socket()
        || token_metadata.file_type().is_symlink()
        || !token_metadata.is_file()
        || token_metadata.nlink() != 1
        || token_metadata.mode() & 0o077 != 0
        || socket_metadata.mode() & 0o007 != 0
        || socket_metadata.uid() != token_metadata.uid()
        || socket_metadata.gid() != token_metadata.gid()
    {
        return Err(Error::new(
            "kernel replay endpoint ownership, type, or mode is invalid",
        ));
    }
    Ok(())
}

fn rejected_handshake(socket: &Path, token: &str, protocol: u8) -> Result<bool> {
    let mut stream = connect(socket)?;
    write_frame(
        &mut stream,
        &json!({
            "token": token,
            "protocol_version": protocol,
            "client_version": "immutable-kernel-replay/1"
        }),
        MAX_HANDSHAKE_BYTES,
    )?;
    let response: HandshakeResponse = read_frame(&mut stream, MAX_HANDSHAKE_BYTES)?;
    Ok(response.status == "error"
        && response.protocol_version == 1
        && !response.server_version.is_empty()
        && response
            .reason
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()))
}

fn authenticated(socket: &Path, token: &str) -> Result<UnixStream> {
    let mut stream = connect(socket)?;
    write_frame(
        &mut stream,
        &json!({
            "token": token,
            "protocol_version": 1,
            "client_version": "immutable-kernel-replay/1"
        }),
        MAX_HANDSHAKE_BYTES,
    )?;
    let response: HandshakeResponse = read_frame(&mut stream, MAX_HANDSHAKE_BYTES)?;
    if response.status != "ok"
        || response.protocol_version != 1
        || response.server_version.is_empty()
        || response.reason.is_some()
    {
        return Err(Error::new("kernel replay authenticated handshake failed"));
    }
    Ok(stream)
}

fn connect(socket: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    Ok(stream)
}

fn write_frame(stream: &mut UnixStream, value: &Value, maximum: usize) -> Result<()> {
    let bytes = canonical_json(value)?;
    if bytes.len() > maximum {
        return Err(Error::new("kernel replay outbound frame exceeds bound"));
    }
    let length = u32::try_from(bytes.len())
        .map_err(|_| Error::new("kernel replay outbound frame length overflow"))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_frame<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
    maximum: usize,
) -> Result<T> {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length)?;
    let length = usize::try_from(u32::from_be_bytes(length))
        .map_err(|_| Error::new("kernel replay inbound frame length overflow"))?;
    if length > maximum {
        return Err(Error::new("kernel replay inbound frame exceeds bound"));
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(Error::from)
}

fn validate_attested_message(message: &Value, session: &str) -> Result<()> {
    let producer = message
        .get("producer")
        .ok_or_else(|| Error::new("kernel replay producer attestation is absent"))?;
    let trace = message
        .get("trace")
        .ok_or_else(|| Error::new("kernel replay trace attestation is absent"))?;
    let valid_uuid = |field: &str| {
        trace
            .get(field)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some_and(|value| !value.is_nil())
    };
    if producer.get("schema_version").and_then(Value::as_u64) != Some(1)
        || producer.get("kind").and_then(Value::as_str) != Some("native_socket_client")
        || !producer
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("native_socket_bridge:"))
        || trace.get("schema_version").and_then(Value::as_u64) != Some(1)
        || !valid_uuid("trace_id")
        || !valid_uuid("turn_id")
        || !valid_uuid("span_id")
        || trace.get("parent_span_id").is_some()
        || trace.get("session_id").and_then(Value::as_str) != Some(session)
        || trace.get("chain_id").is_some()
        || message.get("local_provider_metrics").is_some()
    {
        return Err(Error::new(
            "kernel replay native ingress was not canonically re-attested",
        ));
    }
    Ok(())
}

pub(crate) fn validate_evidence(evidence: &KernelIngressEvidence) -> Result<()> {
    if evidence.schema != "astrid.edge_rescue_helper.kernel_ingress_replay.v1"
        || evidence.provenance
            != "immutable_public_protocol_regression_machine_evidence_not_astrid_authorship"
        || evidence.daemon_sha256.len() != 64
        || !evidence.invalid_token_rejected
        || !evidence.incompatible_protocol_rejected
        || !evidence.authenticated_observer
        || !evidence.authenticated_emitter
        || !evidence.producer_claim_overwritten
        || !evidence.malformed_trace_rerooted
        || !evidence.sensory_mirror_preserved_trace
        || !evidence.forged_provider_metrics_removed
        || !evidence.public_protocol_regression_only
        || evidence.grants_activation_authority
        || evidence.production_continuity_or_reservoir_admission
        || !crate::config::valid_hex64(&evidence.evidence_sha256)
        || evidence.evidence_sha256 != evidence_digest(evidence)?
    {
        return Err(Error::new("kernel ingress replay evidence is incomplete"));
    }
    Ok(())
}

pub(crate) fn evidence_digest(evidence: &KernelIngressEvidence) -> Result<String> {
    let mut value = serde_json::to_value(evidence)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("kernel replay evidence is not an object"))?
        .remove("evidence_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

#[cfg(test)]
mod tests {
    use super::{
        KernelIngressEvidence, evidence_digest, read_frame, validate_attested_message,
        validate_endpoint, validate_evidence, write_frame,
    };
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};

    fn trace() -> serde_json::Value {
        json!({
            "schema_version": 1,
            "trace_id": "11111111-1111-4111-8111-111111111111",
            "turn_id": "22222222-2222-4222-8222-222222222222",
            "span_id": "33333333-3333-4333-8333-333333333333",
            "session_id": "shadow-session"
        })
    }

    #[test]
    fn canonical_native_ingress_shape_is_exact() {
        let message = json!({
            "producer": {
                "schema_version": 1,
                "kind": "native_socket_client",
                "id": "native_socket_bridge:44444444-4444-4444-8444-444444444444"
            },
            "trace": trace()
        });
        assert!(validate_attested_message(&message, "shadow-session").is_ok());
        let mut forged = message;
        forged["producer"]["kind"] = json!("wasm_capsule");
        assert!(validate_attested_message(&forged, "shadow-session").is_err());
    }

    #[test]
    fn regression_evidence_never_claims_authority_or_admission() {
        let mut evidence = KernelIngressEvidence {
            schema: "astrid.edge_rescue_helper.kernel_ingress_replay.v1",
            provenance: "immutable_public_protocol_regression_machine_evidence_not_astrid_authorship",
            daemon_sha256: "a".repeat(64),
            invalid_token_rejected: true,
            incompatible_protocol_rejected: true,
            authenticated_observer: true,
            authenticated_emitter: true,
            producer_claim_overwritten: true,
            malformed_trace_rerooted: true,
            sensory_mirror_preserved_trace: true,
            forged_provider_metrics_removed: true,
            public_protocol_regression_only: true,
            grants_activation_authority: false,
            production_continuity_or_reservoir_admission: false,
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = evidence_digest(&evidence).unwrap();
        assert!(validate_evidence(&evidence).is_ok());
        let mut overclaim = evidence;
        overclaim.grants_activation_authority = true;
        assert!(validate_evidence(&overclaim).is_err());
    }

    #[test]
    fn bounded_frame_codec_is_exact() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let value = json!({"schema":"fixture.v1","value":7});
        write_frame(&mut writer, &value, 1_024).unwrap();
        let decoded: serde_json::Value = read_frame(&mut reader, 1_024).unwrap();
        assert_eq!(decoded, value);
        assert!(write_frame(&mut writer, &value, 4).is_err());
    }

    #[test]
    fn endpoint_requires_a_private_single_link_token_and_unix_socket() {
        let temporary = tempfile::tempdir().unwrap();
        let socket = temporary.path().join("system.sock");
        let token = temporary.path().join("system.token");
        let _listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o660)).unwrap();
        fs::write(&token, "a".repeat(64)).unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(validate_endpoint(&socket, &token).is_ok());
        fs::set_permissions(&token, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(validate_endpoint(&socket, &token).is_err());
    }
}
