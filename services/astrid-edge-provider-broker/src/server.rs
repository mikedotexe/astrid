use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::fd::AsFd as _;
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use fs2::FileExt as _;
use serde::Deserialize;
use serde_json::Value;

use crate::auth::{
    Quotas, RUNTIME_CLIENT, ReplayGuard, STEWARD_CLIENT, WARMUP_CLIENT, fresh_nonce,
    request_signature,
};
use crate::http::{AuthenticatedRequest, Operation, read_request};
use crate::receipt::{Receipt, append, body_hash, now_ms};
use crate::upstream;
use crate::{BROKER_AUTHORITY, Config, Error, Result, WARMUP_PATH};

#[derive(Clone)]
struct Admission(Arc<AtomicUsize>);

impl Admission {
    fn acquire(&self) -> Result<AdmissionGuard> {
        self.0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::new("provider broker is busy"))?;
        Ok(AdmissionGuard(Arc::clone(&self.0)))
    }
}

struct AdmissionGuard(Arc<AtomicUsize>);

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        self.0.store(0, Ordering::Release);
    }
}

/// Serve the immutable provider protocol on the systemd-activated Unix socket.
///
/// # Errors
///
/// Returns an error when the listener identity is wrong or the accept loop fails.
pub fn run(config: &Config, listener_client: &str, credential_directory: &Path) -> Result<()> {
    let listener = activated_listener_from_stdin(config, listener_client)?;
    let _ = config.request_key(listener_client, credential_directory)?;
    let _ = config.ledger_key(listener_client, credential_directory)?;
    let replay = Arc::new(ReplayGuard::default());
    let quotas = Arc::new(Quotas::default());
    let admission = Admission(Arc::new(AtomicUsize::new(0)));
    for incoming in listener.incoming() {
        let mut socket = incoming?;
        socket.set_read_timeout(Some(Duration::from_millis(config.client_read_timeout_ms)))?;
        socket.set_write_timeout(Some(Duration::from_millis(config.client_write_timeout_ms)))?;
        let peer = peer_credentials(&socket)?;
        let Ok(slot) = admission.acquire() else {
            send_error(&mut socket, 503, "busy")?;
            continue;
        };
        let config = config.clone();
        let listener_client = listener_client.to_owned();
        let credential_directory = credential_directory.to_owned();
        let replay = Arc::clone(&replay);
        let quotas = Arc::clone(&quotas);
        std::thread::Builder::new()
            .name("astrid-edge-provider-request".to_owned())
            .spawn(move || {
                let started = Instant::now();
                let context = ConnectionContext {
                    config: &config,
                    listener_client: &listener_client,
                    credential_directory: &credential_directory,
                    replay: &replay,
                    quotas: &quotas,
                };
                let mut response_started = false;
                if let Err(error) =
                    handle_connection(context, &mut socket, peer, started, &mut response_started)
                    && !response_started
                {
                    let _ = send_error(&mut socket, status_for_error(&error), error.code());
                }
                let _ = socket.shutdown(std::net::Shutdown::Both);
                drop(slot);
            })
            .map_err(|error| {
                Error::new(format!("cannot start provider request worker: {error}"))
            })?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Peer {
    uid: u32,
    pid: i32,
}

#[derive(Clone, Copy)]
struct ConnectionContext<'a> {
    config: &'a Config,
    listener_client: &'a str,
    credential_directory: &'a Path,
    replay: &'a ReplayGuard,
    quotas: &'a Quotas,
}

fn handle_connection(
    context: ConnectionContext<'_>,
    socket: &mut UnixStream,
    peer: Peer,
    started: Instant,
    response_started: &mut bool,
) -> Result<()> {
    let request = read_request(
        socket,
        context.config,
        context.listener_client,
        context.credential_directory,
        context.replay,
    )?;
    let client = context.config.client(&request.client_id)?;
    if peer.uid != client.expected_peer_uid || peer.pid <= 0 {
        return Err(Error::new("provider Unix peer identity is unauthorized"));
    }
    context
        .quotas
        .accept(&request.client_id, client.maximum_requests_per_hour)?;
    let _model_guard = authorize_operation(context.config, &request, peer)?;
    let transaction = upstream::transact(
        context.config,
        request.operation,
        &request.body,
        socket,
        response_started,
    );
    match transaction {
        Ok(result) => {
            append(
                context.config,
                context.listener_client,
                context.credential_directory,
                &Receipt {
                    schema: "astrid.edge.provider_broker.receipt.v1",
                    appliance_id: &context.config.appliance_id,
                    recorded_at_unix_ms: now_ms()?,
                    client_id: &request.client_id,
                    operation: request.operation.name(),
                    request_hash: &request.request_hash,
                    status: "completed",
                    http_status: Some(result.status),
                    response_body_sha256: Some(&result.response_body_sha256),
                    response_body_bytes: Some(result.response_body_bytes),
                    elapsed_ms: result.elapsed_ms,
                    authority: "immutable_inference_gateway_observation_only",
                },
            )?;
            Ok(())
        },
        Err(error) => {
            let _ = append(
                context.config,
                context.listener_client,
                context.credential_directory,
                &Receipt {
                    schema: "astrid.edge.provider_broker.receipt.v1",
                    appliance_id: &context.config.appliance_id,
                    recorded_at_unix_ms: now_ms().unwrap_or(0),
                    client_id: &request.client_id,
                    operation: request.operation.name(),
                    request_hash: &request.request_hash,
                    status: error.code(),
                    http_status: None,
                    response_body_sha256: None,
                    response_body_bytes: None,
                    elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    authority: "immutable_inference_gateway_observation_only",
                },
            );
            Err(error)
        },
    }
}

enum ModelGuard {
    Held(File),
    StewardVerified,
}

fn authorize_operation(
    config: &Config,
    request: &AuthenticatedRequest,
    peer: Peer,
) -> Result<ModelGuard> {
    validate_lease_path(&config.maintenance_lease)?;
    validate_lease_path(&config.reflection_lease)?;
    match request.operation {
        Operation::Inference if request.client_id == RUNTIME_CLIENT => {
            if config.maintenance_lease.exists() || config.reflection_lease.exists() {
                return Err(Error::new(
                    "runtime inference denied by maintenance or reflection lease",
                ));
            }
            let lock = open_model_lock(&config.model_lock)?;
            lock.try_lock_exclusive()
                .map_err(|_| Error::new("runtime provider model lock is busy"))?;
            Ok(ModelGuard::Held(lock))
        },
        Operation::Inference | Operation::Unload if request.client_id == STEWARD_CLIENT => {
            require_live_reflection(config)?;
            if config.maintenance_lease.exists() {
                return Err(Error::new("steward inference denied by maintenance lease"));
            }
            if !peer_holds_model_lock(peer.pid, &config.model_lock)? {
                return Err(Error::new(
                    "steward peer does not hold the exact model lock",
                ));
            }
            Ok(ModelGuard::StewardVerified)
        },
        Operation::Warmup if request.client_id == WARMUP_CLIENT => {
            if config.reflection_lease.exists() || config.maintenance_lease.exists() {
                return Err(Error::new(
                    "warmup denied during scheduled reflection or maintenance",
                ));
            }
            let lock = open_model_lock(&config.model_lock)?;
            lock.try_lock_exclusive()
                .map_err(|_| Error::new("warmup provider model lock is busy"))?;
            Ok(ModelGuard::Held(lock))
        },
        _ => Err(Error::new(
            "provider operation escaped client authorization",
        )),
    }
}

impl Drop for ModelGuard {
    fn drop(&mut self) {
        if let Self::Held(file) = self {
            let _ = fs2::FileExt::unlock(file);
        }
    }
}

fn open_model_lock(path: &Path) -> Result<File> {
    let before = fs::symlink_metadata(path)?;
    validate_model_lock(&before)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000 | 0o02_000_000);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    validate_model_lock(&opened)?;
    validate_model_lock(&after)?;
    let identity = |metadata: &fs::Metadata| (metadata.dev(), metadata.ino());
    if identity(&before) != identity(&opened) || identity(&opened) != identity(&after) {
        return Err(Error::new("provider model lock changed during open"));
    }
    Ok(file)
}

fn validate_model_lock(metadata: &fs::Metadata) -> Result<()> {
    let expected_uid = if cfg!(debug_assertions) {
        nix::unistd::geteuid().as_raw()
    } else {
        0
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o777 != 0o640
    {
        return Err(Error::new("provider model lock identity is invalid"));
    }
    Ok(())
}

fn validate_lease_path(path: &Path) -> Result<()> {
    if !path.exists() && !path.is_symlink() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)?;
    let expected_uid = if cfg!(debug_assertions) {
        nix::unistd::geteuid().as_raw()
    } else {
        0
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.len() == 0
        || metadata.len() > 64 * 1024
    {
        return Err(Error::new("provider lease identity is invalid"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReflectionLease {
    schema: String,
    lease_kind: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    reason: String,
    owner: String,
    lease_id: String,
    nonce: String,
    host_boot_id: String,
    service_invocation_id: String,
    generation_id: String,
}

fn require_live_reflection(config: &Config) -> Result<()> {
    validate_lease_path(&config.reflection_lease)?;
    let body = fs::read(&config.reflection_lease)?;
    let lease: ReflectionLease = serde_json::from_slice(&body)?;
    let now = now_ms()?;
    if lease.schema != "astrid.edge_scheduled_reflection.lease.v1"
        || lease.lease_kind != "scheduled_reflection"
        || lease.owner != "immutable_astrid_edge_reflection_guard"
        || lease.created_at_unix_ms > now
        || lease.expires_at_unix_ms <= now
        || lease
            .expires_at_unix_ms
            .saturating_sub(lease.created_at_unix_ms)
            > 10_800_000
        || [
            lease.reason,
            lease.lease_id,
            lease.nonce,
            lease.host_boot_id,
            lease.service_invocation_id,
            lease.generation_id,
        ]
        .iter()
        .any(|value| value.is_empty() || value.len() > 256)
    {
        return Err(Error::new(
            "reflection lease is absent, stale, or malformed",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn peer_holds_model_lock(pid: i32, path: &Path) -> Result<bool> {
    let metadata = fs::metadata(path)?;
    let inode = metadata.ino().to_string();
    let pid = pid.to_string();
    let locks = fs::read_to_string("/proc/locks")?;
    Ok(locks.lines().any(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        fields.len() >= 6
            && fields[1] == "FLOCK"
            && fields[3] == "WRITE"
            && fields[4] == pid
            && fields[5].rsplit(':').next() == Some(inode.as_str())
    }))
}

#[cfg(not(target_os = "linux"))]
fn peer_holds_model_lock(_pid: i32, _path: &Path) -> Result<bool> {
    Err(Error::new(
        "steward model-lock proof requires Linux /proc/locks",
    ))
}

fn activated_listener_from_stdin(config: &Config, listener_client: &str) -> Result<UnixListener> {
    let stdin = std::io::stdin();
    let descriptor = stdin.as_fd().try_clone_to_owned().map_err(|error| {
        Error::new(format!(
            "cannot duplicate activated provider listener: {error}"
        ))
    })?;
    let listener = UnixListener::from(descriptor);
    let address = listener.local_addr()?;
    let client = config.client(listener_client)?;
    if address.as_pathname() != Some(client.socket_path.as_path()) {
        return Err(Error::new("activated provider listener path is not exact"));
    }
    let metadata = fs::symlink_metadata(&client.socket_path)?;
    if !std::os::unix::fs::FileTypeExt::is_socket(&metadata.file_type())
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != 0
        || metadata.gid() != client.socket_gid
        || metadata.permissions().mode() & 0o7777 != 0o660
    {
        return Err(Error::new("activated provider socket identity is invalid"));
    }
    Ok(listener)
}

#[cfg(target_os = "linux")]
fn peer_credentials(socket: &UnixStream) -> Result<Peer> {
    let credentials =
        nix::sys::socket::getsockopt(socket, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(|error| Error::new(format!("cannot read provider Unix peer: {error}")))?;
    Ok(Peer {
        uid: credentials.uid(),
        pid: credentials.pid(),
    })
}

#[cfg(not(target_os = "linux"))]
fn peer_credentials(_socket: &UnixStream) -> Result<Peer> {
    Err(Error::new(
        "provider Unix peer authentication requires Linux SO_PEERCRED",
    ))
}

fn send_error(socket: &mut UnixStream, status: u16, code: &str) -> Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": {"code": code, "message": "immutable provider gateway denied the request"}
    }))?;
    write!(
        socket,
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        if status == 503 {
            "Service Unavailable"
        } else {
            "Forbidden"
        },
        body.len()
    )?;
    socket.write_all(&body)?;
    socket.flush()?;
    Ok(())
}

fn status_for_error(error: &Error) -> u16 {
    match error.code() {
        "busy" => 503,
        "upstream_error" => 502,
        _ => 403,
    }
}

/// Issue the fixed warmup operation through the immutable Unix gateway.
///
/// # Errors
///
/// Returns an error for credential, transport, gateway, or receipt persistence failure.
pub fn run_warmup_client(config: &Config, key_path: &Path, receipt_path: &Path) -> Result<()> {
    let started = Instant::now();
    let key = config.warmup_client_key(key_path)?;
    let body = serde_json::to_vec(&serde_json::json!({
        "schema": "astrid.edge.provider_broker.warmup.v1",
        "model": config.model,
    }))?;
    let nonce = fresh_nonce()?;
    let auth = request_signature(&key, WARMUP_CLIENT, WARMUP_PATH, &nonce, &body)?;
    let mut socket = UnixStream::connect(&config.warmup.socket_path)?;
    socket.set_read_timeout(Some(Duration::from_millis(config.total_timeout_ms)))?;
    socket.set_write_timeout(Some(Duration::from_millis(config.client_write_timeout_ms)))?;
    write!(
        socket,
        "POST {WARMUP_PATH} HTTP/1.1\r\nHost: {BROKER_AUTHORITY}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Provider-Client: {WARMUP_CLIENT}\r\nX-Astrid-Provider-Nonce: {nonce}\r\nX-Astrid-Provider-Auth: {auth}\r\n\r\n",
        body.len()
    )?;
    socket.write_all(&body)?;
    socket.flush()?;
    let maximum_response = config.maximum_response_body_bytes.min(1_048_576);
    let mut response = Vec::new();
    socket
        .take(maximum_response.saturating_add(1))
        .read_to_end(&mut response)?;
    if u64::try_from(response.len()).unwrap_or(u64::MAX) > maximum_response {
        return Err(Error::new(
            "immutable provider warmup response exceeded its bound",
        ));
    }
    if !response.starts_with(b"HTTP/1.1 200 ") {
        return Err(Error::new("immutable provider warmup did not succeed"));
    }
    let canary = verify_canonical_canary_response(&response, &config.model)?;
    let completed = now_ms()?;
    let warmup = serde_json::to_vec_pretty(&serde_json::json!({
        "schema": "astrid_edge_model_warmup_v3",
        "model": config.model,
        "status": "loaded_and_canary_verified_via_immutable_provider_gateway",
        "completed_at_unix_ms": completed,
        "elapsed_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "keep_alive": config.keep_alive,
        "gateway_wire_sha256": body_hash(&response),
        "provider_body_sha256": canary.provider_body_sha256,
        "model_response_sha256": canary.model_response_sha256,
        "model_response_bytes": canary.model_response_bytes,
        "canonical_response_verified": true,
        "authority": "immutable_non_authored_non_continuity_non_reservoir_provider_canary",
    }))?;
    let parent = receipt_path
        .parent()
        .ok_or_else(|| Error::new("warmup receipt has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = receipt_path.with_extension("json.tmp");
    fs::write(&temporary, warmup)?;
    fs::rename(temporary, receipt_path)?;
    Ok(())
}

struct CanonicalCanary {
    provider_body_sha256: String,
    model_response_sha256: String,
    model_response_bytes: u64,
}

fn verify_canonical_canary_response(
    response: &[u8],
    expected_model: &str,
) -> Result<CanonicalCanary> {
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| Error::new("provider gateway response headers are incomplete"))?;
    let header_end = split.saturating_add(4);
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|_| Error::new("provider gateway response headers are not ASCII"))?;
    if !header.is_ascii() {
        return Err(Error::new(
            "provider gateway response headers are not ASCII",
        ));
    }
    let mut lines = header.split("\r\n");
    if lines.next() != Some("HTTP/1.1 200 OK") {
        return Err(Error::new(
            "provider gateway response status is not canonical",
        ));
    }
    let mut headers = std::collections::BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new("provider gateway response header is malformed"))?;
        if headers
            .insert(name.to_ascii_lowercase(), value.trim())
            .is_some()
        {
            return Err(Error::new("provider gateway response header is duplicated"));
        }
    }
    if headers.len() != 4
        || headers.get("content-type") != Some(&"application/json")
        || headers.get("connection") != Some(&"close")
        || headers.get("transfer-encoding") != Some(&"chunked")
        || headers.get("x-astrid-provider-gateway") != Some(&"immutable-v1")
    {
        return Err(Error::new(
            "provider gateway response headers differ from the immutable protocol",
        ));
    }
    let body = decode_canonical_chunked_body(&response[header_end..])?;
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| Error::new("provider canary response is not JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("provider canary response is not an object"))?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("provider canary response has no model identity"))?;
    let model_response = object
        .get("response")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("provider canary response has no model output"))?;
    let thinking_is_empty = object
        .get("thinking")
        .is_none_or(|thinking| thinking.as_str().is_some_and(str::is_empty));
    if model != expected_model
        || object.get("done").and_then(Value::as_bool) != Some(true)
        || object.contains_key("error")
        || model_response.trim() != "OK"
        || model_response.len() > 8
        || !thinking_is_empty
    {
        return Err(Error::new(
            "provider canary did not return the exact bounded canonical response",
        ));
    }
    Ok(CanonicalCanary {
        provider_body_sha256: body_hash(&body),
        model_response_sha256: body_hash(model_response.as_bytes()),
        model_response_bytes: u64::try_from(model_response.len()).unwrap_or(u64::MAX),
    })
}

fn decode_canonical_chunked_body(wire: &[u8]) -> Result<Vec<u8>> {
    const MAXIMUM_CANARY_BODY_BYTES: usize = 64 * 1024;
    let mut cursor = 0_usize;
    let mut body = Vec::new();
    loop {
        let rest = wire
            .get(cursor..)
            .ok_or_else(|| Error::new("provider gateway chunk cursor escaped response"))?;
        let line_end = rest
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| Error::new("provider gateway chunk header is incomplete"))?;
        if line_end == 0 || line_end > 16 {
            return Err(Error::new("provider gateway chunk header is not bounded"));
        }
        let size_text = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| Error::new("provider gateway chunk size is not ASCII"))?;
        if size_text.bytes().any(|byte| !byte.is_ascii_hexdigit()) {
            return Err(Error::new("provider gateway chunk size is not canonical"));
        }
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| Error::new("provider gateway chunk size is invalid"))?;
        cursor = cursor.saturating_add(line_end).saturating_add(2);
        if size == 0 {
            if wire.get(cursor..) != Some(b"\r\n".as_slice()) {
                return Err(Error::new(
                    "provider gateway chunked response has trailers or trailing bytes",
                ));
            }
            return Ok(body);
        }
        if body.len().saturating_add(size) > MAXIMUM_CANARY_BODY_BYTES {
            return Err(Error::new("provider canary body exceeds immutable bound"));
        }
        let end = cursor
            .checked_add(size)
            .ok_or_else(|| Error::new("provider gateway chunk length overflow"))?;
        let chunk = wire
            .get(cursor..end)
            .ok_or_else(|| Error::new("provider gateway chunk body is incomplete"))?;
        if wire.get(end..end.saturating_add(2)) != Some(b"\r\n".as_slice()) {
            return Err(Error::new("provider gateway chunk terminator is invalid"));
        }
        body.extend_from_slice(chunk);
        cursor = end.saturating_add(2);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;
    use std::time::{Duration, Instant};

    #[cfg(target_os = "linux")]
    use fs2::FileExt as _;

    use super::{
        Admission, AuthenticatedRequest, ModelGuard, Operation, Peer, ReplayGuard,
        authorize_operation, verify_canonical_canary_response,
    };
    #[cfg(target_os = "linux")]
    use super::{ConnectionContext, Quotas, handle_connection, peer_credentials};
    #[cfg(target_os = "linux")]
    use crate::auth::request_signature;

    fn inference(client: &str) -> AuthenticatedRequest {
        AuthenticatedRequest {
            operation: Operation::Inference,
            client_id: client.to_owned(),
            request_hash: "a".repeat(64),
            body: br#"{"model":"qwen3.5:4b"}"#.to_vec(),
        }
    }

    #[cfg(target_os = "linux")]
    fn valid_wire(body: &[u8], nonce_suffix: char) -> Vec<u8> {
        let nonce = format!(
            "{:016x}{}",
            crate::receipt::now_ms().unwrap(),
            nonce_suffix.to_string().repeat(48)
        );
        let auth = request_signature(
            &[1; 32],
            "edge-runtime",
            crate::INFERENCE_PATH,
            &nonce,
            body,
        )
        .unwrap();
        format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Provider-Client: edge-runtime\r\nX-Astrid-Provider-Nonce: {nonce}\r\nX-Astrid-Provider-Auth: {auth}\r\n\r\n",
            crate::INFERENCE_PATH,
            crate::BROKER_AUTHORITY,
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    #[test]
    fn separate_endpoint_admission_cannot_starve_steward_or_warmup() {
        let runtime = Admission(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        let steward = Admission(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        let warmup = Admission(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        let runtime_guard = runtime.acquire().unwrap();
        assert!(runtime.acquire().is_err());
        let _steward_guard = steward.acquire().unwrap();
        let _warmup_guard = warmup.acquire().unwrap();
        drop(runtime_guard);
        assert!(runtime.acquire().is_ok());
    }

    #[test]
    fn warmup_canary_accepts_only_exact_gateway_and_model_response() {
        fn response(body: &[u8]) -> Vec<u8> {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nX-Astrid-Provider-Gateway: immutable-v1\r\n\r\n{:x}\r\n",
                body.len()
            )
            .into_bytes()
            .into_iter()
            .chain(body.iter().copied())
            .chain(b"\r\n0\r\n\r\n".iter().copied())
            .collect()
        }

        let body = br#"{"model":"qwen3.5:4b","response":"OK","thinking":"","done":true}"#;
        let canary = verify_canonical_canary_response(&response(body), "qwen3.5:4b").unwrap();
        assert_eq!(canary.model_response_bytes, 2);
        assert_eq!(
            canary.model_response_sha256,
            crate::receipt::body_hash(b"OK")
        );

        let wrong = br#"{"model":"qwen3.5:4b","response":"almost OK","done":true}"#;
        assert!(verify_canonical_canary_response(&response(wrong), "qwen3.5:4b").is_err());
        let thinking = br#"{"model":"qwen3.5:4b","response":"OK","thinking":"hidden","done":true}"#;
        assert!(verify_canonical_canary_response(&response(thinking), "qwen3.5:4b").is_err());

        let mut trailing = response(body);
        trailing.extend_from_slice(b"untrusted");
        assert!(verify_canonical_canary_response(&trailing, "qwen3.5:4b").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unix_peer_credentials_are_kernel_reported_and_wrong_uid_is_rejected() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let peer = peer_credentials(&server).unwrap();
        assert_eq!(peer.uid, nix::unistd::geteuid().as_raw());
        assert_eq!(peer.pid, i32::try_from(std::process::id()).unwrap());

        let body = br#"{"model":"qwen3.5:4b","messages":[{"role":"user","content":"hi"}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64}"#;
        client.write_all(&valid_wire(body, 'b')).unwrap();
        let (config, credentials) = crate::config::tests_support::config_for_protocol_tests();
        let wrong_peer = Peer {
            uid: config.runtime.expected_peer_uid.saturating_add(1),
            pid: peer.pid,
        };
        let context = ConnectionContext {
            config: &config,
            listener_client: "edge-runtime",
            credential_directory: credentials.path(),
            replay: &ReplayGuard::default(),
            quotas: &Quotas::default(),
        };
        let result =
            handle_connection(context, &mut server, wrong_peer, Instant::now(), &mut false);
        assert!(result.is_err());
    }

    #[test]
    fn incomplete_unauthenticated_request_is_bounded_by_socket_deadline() {
        let (_client, mut server) = UnixStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(25)))
            .unwrap();
        let (config, credentials) = crate::config::tests_support::config_for_protocol_tests();
        let started = Instant::now();
        let result = crate::http::read_request(
            &mut server,
            &config,
            "edge-runtime",
            credentials.path(),
            &ReplayGuard::default(),
        );
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn runtime_lock_is_exclusive_released_and_immediately_reusable() {
        let (config, _temporary) = crate::config::tests_support::config_for_protocol_tests();
        let request = inference("edge-runtime");
        let first = authorize_operation(
            &config,
            &request,
            Peer {
                uid: config.runtime.expected_peer_uid,
                pid: i32::try_from(std::process::id()).unwrap(),
            },
        )
        .unwrap();
        assert!(matches!(first, ModelGuard::Held(_)));
        assert!(
            authorize_operation(
                &config,
                &request,
                Peer {
                    uid: config.runtime.expected_peer_uid,
                    pid: i32::try_from(std::process::id()).unwrap(),
                },
            )
            .is_err()
        );
        drop(first);
        assert!(
            authorize_operation(
                &config,
                &request,
                Peer {
                    uid: config.runtime.expected_peer_uid,
                    pid: i32::try_from(std::process::id()).unwrap(),
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn warmup_fails_closed_for_either_reflection_or_maintenance_lease() {
        let (config, _temporary) = crate::config::tests_support::config_for_protocol_tests();
        let request = AuthenticatedRequest {
            operation: Operation::Warmup,
            client_id: "model-warmup".to_owned(),
            request_hash: "b".repeat(64),
            body: Vec::new(),
        };
        std::fs::write(&config.maintenance_lease, b"maintenance").unwrap();
        assert!(
            authorize_operation(
                &config,
                &request,
                Peer {
                    uid: config.warmup.expected_peer_uid,
                    pid: i32::try_from(std::process::id()).unwrap(),
                },
            )
            .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn steward_must_hold_exact_model_lock_under_a_live_reflection_lease() {
        let (config, _temporary) = crate::config::tests_support::config_for_protocol_tests();
        let now = crate::receipt::now_ms().unwrap();
        std::fs::write(
            &config.reflection_lease,
            serde_json::to_vec(&serde_json::json!({
                "schema":"astrid.edge_scheduled_reflection.lease.v1",
                "lease_kind":"scheduled_reflection",
                "created_at_unix_ms":now,
                "expires_at_unix_ms":now.saturating_add(60_000),
                "reason":"scheduled",
                "owner":"immutable_astrid_edge_reflection_guard",
                "lease_id":"lease",
                "nonce":"nonce",
                "host_boot_id":"boot",
                "service_invocation_id":"service",
                "generation_id":"generation"
            }))
            .unwrap(),
        )
        .unwrap();
        let request = inference("edge-steward");
        let peer = Peer {
            uid: config.steward.expected_peer_uid,
            pid: i32::try_from(std::process::id()).unwrap(),
        };
        assert!(authorize_operation(&config, &request, peer).is_err());
        let lock = std::fs::File::open(&config.model_lock).unwrap();
        lock.try_lock_exclusive().unwrap();
        assert!(authorize_operation(&config, &request, peer).is_ok());
    }
}
