use std::fs;
use std::io::Write;
use std::net::Shutdown;
use std::os::fd::AsFd as _;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use serde::Serialize;

use crate::auth::{ReplayGuard, response_signature};
use crate::http::{BrokerRequest, read_broker_request};
use crate::quota::PersistentSearchQuota;
use crate::{BraveSearch, Config, Error, FetchBackend, Result, SearchBackend};

trait WebBackend: SearchBackend + FetchBackend {}

impl<T: SearchBackend + FetchBackend> WebBackend for T {}

#[derive(Clone, Debug)]
pub struct Admission {
    active: Arc<AtomicUsize>,
    maximum: usize,
}

impl Admission {
    #[must_use]
    pub fn new(maximum: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            maximum,
        }
    }

    /// Acquire one bounded request slot without waiting.
    ///
    /// # Errors
    ///
    /// Returns `busy` once the exact configured concurrency limit is active.
    pub fn try_acquire(&self) -> Result<AdmissionGuard> {
        loop {
            let current = self.active.load(Ordering::Acquire);
            if current >= self.maximum {
                return Err(Error::new("broker is busy"));
            }
            let next = current.saturating_add(1);
            if self
                .active
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(AdmissionGuard {
                    active: Arc::clone(&self.active),
                });
            }
        }
    }

    #[must_use]
    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
}

pub struct AdmissionGuard {
    active: Arc<AtomicUsize>,
}

struct ConnectionSecurity<'a> {
    request_key: &'a [u8; 32],
    response_signing_key: &'a SigningKey,
    replay: &'a ReplayGuard,
    request_admission: &'a Admission,
    search_quota: &'a PersistentSearchQuota,
}

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        let _ = self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_sub(1)
            });
    }
}

fn activated_listener_from_stdin(config: &Config) -> Result<UnixListener> {
    let stdin = std::io::stdin();
    let descriptor = stdin
        .as_fd()
        .try_clone_to_owned()
        .map_err(|error| Error::new(format!("cannot duplicate activated listener: {error}")))?;
    let listener = UnixListener::from(descriptor);
    validate_activated_listener(&listener, &config.socket_path, config.socket_gid, 0)?;
    Ok(listener)
}

fn validate_activated_listener(
    listener: &UnixListener,
    expected_path: &Path,
    required_socket_gid: u32,
    required_owner_uid: u32,
) -> Result<()> {
    let address = listener.local_addr()?;
    if address.as_pathname() != Some(expected_path) {
        return Err(Error::new(
            "activated listener path differs from immutable configuration",
        ));
    }
    let parent = expected_path
        .parent()
        .ok_or_else(|| Error::new("activated listener has no parent"))?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != required_owner_uid
        || parent_metadata.permissions().mode() & 0o022 != 0
    {
        return Err(Error::new(
            "activated listener parent is not immutable and root-owned",
        ));
    }
    let metadata = fs::symlink_metadata(expected_path)?;
    if !metadata.file_type().is_socket()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != required_owner_uid
        || metadata.gid() != required_socket_gid
        || metadata.permissions().mode() & 0o7777 != 0o660
    {
        return Err(Error::new(
            "activated listener ownership, mode, link count, or type is invalid",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn peer_uid(socket: &UnixStream) -> Result<u32> {
    let credentials =
        nix::sys::socket::getsockopt(socket, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(|error| Error::new(format!("cannot read Unix peer credentials: {error}")))?;
    let uid = credentials.uid();
    if credentials.pid() <= 0 {
        return Err(Error::new("Unix peer credentials omitted a valid PID"));
    }
    Ok(uid)
}

#[cfg(not(target_os = "linux"))]
fn peer_uid(_socket: &UnixStream) -> Result<u32> {
    Err(Error::new(
        "SO_PEERCRED is required and available only on the Linux appliance target",
    ))
}

/// Run the immutable broker until its process is stopped by the service manager.
///
/// # Errors
///
/// Returns an error if the fixed client cannot be built, the exact
/// systemd-activated Unix listener cannot be verified, or accepting a
/// connection fails.
pub fn run(config: &Config) -> Result<()> {
    let backend = Arc::new(BraveSearch::new(config.clone())?);
    let listener = activated_listener_from_stdin(config)?;
    run_with_backend(config, &backend, &listener)
}

fn run_with_backend<B: WebBackend>(
    config: &Config,
    backend: &Arc<B>,
    listener: &UnixListener,
) -> Result<()> {
    let request_key = Arc::new(config.request_key()?);
    let response_signing_key = Arc::new(config.response_signing_key()?);
    let replay = Arc::new(ReplayGuard::default());
    let search_quota = Arc::new(PersistentSearchQuota::open(config)?);
    let maximum = usize::from(config.maximum_concurrent_requests);
    let authentication_admission = Admission::new(maximum);
    let request_admission = Arc::new(Admission::new(maximum));
    for incoming in listener.incoming() {
        let mut socket = incoming?;
        socket.set_read_timeout(Some(Duration::from_millis(config.client_read_timeout_ms)))?;
        socket.set_write_timeout(Some(Duration::from_millis(config.client_write_timeout_ms)))?;
        if peer_uid(&socket)? != config.expected_peer_uid {
            send_unauthenticated_error(&mut socket, 403, "invalid_request")?;
            continue;
        }
        let Ok(authentication_guard) = authentication_admission.try_acquire() else {
            send_unauthenticated_error(&mut socket, 503, "busy")?;
            continue;
        };
        let config = config.clone();
        let backend = Arc::clone(backend);
        let replay = Arc::clone(&replay);
        let request_key = Arc::clone(&request_key);
        let response_signing_key = Arc::clone(&response_signing_key);
        let request_admission = Arc::clone(&request_admission);
        let search_quota = Arc::clone(&search_quota);
        thread::Builder::new()
            .name("astrid-edge-web-request".to_string())
            .spawn(move || {
                if let Err(error) = handle_connection(
                    &mut socket,
                    &config,
                    backend.as_ref(),
                    authentication_guard,
                    &ConnectionSecurity {
                        request_key: &request_key,
                        response_signing_key: &response_signing_key,
                        replay: &replay,
                        request_admission: &request_admission,
                        search_quota: &search_quota,
                    },
                ) {
                    let _ = send_unauthenticated_error(&mut socket, 400, error.code());
                }
                let _ = socket.shutdown(Shutdown::Both);
            })
            .map_err(|error| {
                Error::new(format!("could not spawn bounded broker worker: {error}"))
            })?;
    }
    Ok(())
}

fn handle_connection<B: WebBackend>(
    socket: &mut UnixStream,
    config: &Config,
    backend: &B,
    authentication_guard: AdmissionGuard,
    security: &ConnectionSecurity<'_>,
) -> Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(config.client_read_timeout_ms)))?;
    socket.set_write_timeout(Some(Duration::from_millis(config.client_write_timeout_ms)))?;
    let authenticated = read_broker_request(socket, config, security.request_key, security.replay)?;
    drop(authentication_guard);
    let _request_guard = security.request_admission.try_acquire()?;
    let client_id = authenticated.client_id;
    let nonce = authenticated.nonce;
    let request_hash = authenticated.request_hash;
    match authenticated.request {
        BrokerRequest::Search(request) => {
            if let Err(error) = security
                .search_quota
                .admit(&request.trace_id, &request_hash)
            {
                return send_authenticated_error(
                    socket,
                    429,
                    error.code(),
                    security.response_signing_key,
                    &client_id,
                    &nonce,
                    &request_hash,
                );
            }
            match backend.search(&request).and_then(|response| {
                response.validate(config, request.limit)?;
                Ok(response)
            }) {
                Ok(response) => send_authenticated_json(
                    socket,
                    200,
                    &response,
                    security.response_signing_key,
                    &client_id,
                    &nonce,
                    &request_hash,
                ),
                Err(error) => send_authenticated_error(
                    socket,
                    400,
                    error.code(),
                    security.response_signing_key,
                    &client_id,
                    &nonce,
                    &request_hash,
                ),
            }
        },
        BrokerRequest::Fetch(request) => {
            match backend.fetch(&request).and_then(|response| {
                response.validate(&request)?;
                Ok(response)
            }) {
                Ok(response) => send_authenticated_json(
                    socket,
                    200,
                    &response,
                    security.response_signing_key,
                    &client_id,
                    &nonce,
                    &request_hash,
                ),
                Err(error) => send_authenticated_error(
                    socket,
                    400,
                    error.code(),
                    security.response_signing_key,
                    &client_id,
                    &nonce,
                    &request_hash,
                ),
            }
        },
    }
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    schema: &'static str,
    code: &'a str,
}

fn send_unauthenticated_error(socket: &mut UnixStream, status: u16, code: &str) -> Result<()> {
    send_json(
        socket,
        status,
        &ErrorResponse {
            schema: "astrid.edge.web_broker.error.v1",
            code,
        },
        None,
    )
}

fn send_authenticated_error(
    socket: &mut UnixStream,
    status: u16,
    code: &str,
    signing_key: &SigningKey,
    client_id: &str,
    nonce: &str,
    request_hash: &str,
) -> Result<()> {
    send_authenticated_json(
        socket,
        status,
        &ErrorResponse {
            schema: "astrid.edge.web_broker.error.v1",
            code,
        },
        signing_key,
        client_id,
        nonce,
        request_hash,
    )
}

fn send_authenticated_json(
    socket: &mut UnixStream,
    status: u16,
    value: &impl Serialize,
    signing_key: &SigningKey,
    client_id: &str,
    nonce: &str,
    request_hash: &str,
) -> Result<()> {
    send_json(
        socket,
        status,
        value,
        Some((signing_key, client_id, nonce, request_hash)),
    )
}

fn send_json(
    socket: &mut UnixStream,
    status: u16,
    value: &impl Serialize,
    authentication: Option<(&SigningKey, &str, &str, &str)>,
) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => return Err(Error::new("broker attempted unsupported response status")),
    };
    write!(
        socket,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n",
        body.len()
    )?;
    if let Some((signing_key, client_id, nonce, request_hash)) = authentication {
        let signature =
            response_signature(signing_key, client_id, nonce, status, request_hash, &body)?;
        write!(
            socket,
            "X-Astrid-Web-Client: {client_id}\r\nX-Astrid-Web-Nonce: {nonce}\r\nX-Astrid-Web-Request-Hash: {request_hash}\r\nX-Astrid-Web-Signature: {signature}\r\n"
        )?;
    }
    write!(socket, "\r\n")?;
    socket.write_all(&body)?;
    socket.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::Arc;
    use std::thread;

    #[cfg(target_os = "linux")]
    use super::peer_uid;
    use super::{Admission, validate_activated_listener};

    #[test]
    fn concurrency_is_exact_and_released_by_guard_drop() {
        let admission = Arc::new(Admission::new(2));
        let first = admission.try_acquire().unwrap();
        let second = admission.try_acquire().unwrap();
        assert_eq!(admission.active(), 2);
        assert!(admission.try_acquire().is_err());
        drop(first);
        assert!(admission.try_acquire().is_ok());
        drop(second);
    }

    #[test]
    fn concurrent_callers_never_exceed_limit() {
        let admission = Arc::new(Admission::new(1));
        let guard = admission.try_acquire().unwrap();
        let other = Arc::clone(&admission);
        let denied = thread::spawn(move || other.try_acquire().is_err())
            .join()
            .unwrap();
        assert!(denied);
        drop(guard);
        assert!(admission.try_acquire().is_ok());
    }

    #[test]
    fn independent_listener_pools_cannot_starve_each_other() {
        let runtime = Admission::new(1);
        let steward = Admission::new(1);
        let runtime_guard = runtime.try_acquire().unwrap();
        assert!(runtime.try_acquire().is_err());
        let steward_guard = steward.try_acquire().unwrap();
        assert!(steward.try_acquire().is_err());
        drop(runtime_guard);
        drop(steward_guard);
    }

    #[test]
    fn activated_listener_requires_exact_path_owner_group_mode_and_type() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("broker.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
        let parent = fs::metadata(temporary.path()).unwrap();
        let socket = fs::metadata(&path).unwrap();
        validate_activated_listener(&listener, &path, socket.gid(), parent.uid()).unwrap();

        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(validate_activated_listener(&listener, &path, socket.gid(), parent.uid()).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
        assert!(
            validate_activated_listener(
                &listener,
                &temporary.path().join("other.sock"),
                socket.gid(),
                parent.uid(),
            )
            .is_err()
        );
    }

    #[test]
    fn activated_listener_clone_survives_the_original_descriptor() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("broker.sock");
        let original = UnixListener::bind(&path).unwrap();
        let activated = original.try_clone().unwrap();
        drop(original);

        let client = UnixStream::connect(&path).unwrap();
        let (accepted, _) = activated.accept().unwrap();
        drop(client);
        drop(accepted);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_peer_credentials_bind_the_connecting_uid() {
        let temporary = tempfile::tempdir().unwrap();
        let expected_uid = fs::metadata(temporary.path()).unwrap().uid();
        let (client, server) = UnixStream::pair().unwrap();
        assert_eq!(peer_uid(&client).unwrap(), expected_uid);
        assert_eq!(peer_uid(&server).unwrap(), expected_uid);
    }
}
