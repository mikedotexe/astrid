use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use socket2::{Domain, SockAddr, Socket, Type};

use crate::config::{Config, validate_loopback_origin};
use crate::{Error, Result};

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const BROKER_AUTHORITY: &str = "astrid-edge-provider";
const BROKER_CLIENT: &str = "edge-steward";
const BROKER_INFERENCE_PATH: &str = "/v1/chat/completions";
const BROKER_UNLOAD_PATH: &str = "/internal/unload";
const BROKER_DOMAIN: &[u8] = b"astrid.edge.provider_broker.request.v1";

enum ProviderStream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl Read for ProviderStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buffer),
            Self::Unix(stream) => stream.read(buffer),
        }
    }
}

impl Read for &ProviderStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ProviderStream::Tcp(stream) => {
                let mut stream = stream;
                stream.read(buffer)
            },
            ProviderStream::Unix(stream) => {
                let mut stream = stream;
                stream.read(buffer)
            },
        }
    }
}

impl Write for ProviderStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buffer),
            Self::Unix(stream) => stream.write(buffer),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            Self::Unix(stream) => stream.flush(),
        }
    }
}

impl ProviderStream {
    fn set_nodelay(&self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.set_nodelay(true),
            Self::Unix(_) => Ok(()),
        }
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.set_read_timeout(timeout),
            Self::Unix(stream) => stream.set_read_timeout(timeout),
        }
    }

    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.set_write_timeout(timeout),
            Self::Unix(stream) => stream.set_write_timeout(timeout),
        }
    }

    fn set_nonblocking(&self, value: bool) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.set_nonblocking(value),
            Self::Unix(stream) => stream.set_nonblocking(value),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub content: String,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone)]
pub struct UnloadResponse {
    pub request_sha256: String,
    pub result_sha256: String,
    pub elapsed_ms: u64,
}

pub struct Provider<'a> {
    config: &'a Config,
}

impl<'a> Provider<'a> {
    #[must_use]
    pub const fn new(config: &'a Config) -> Self {
        Self { config }
    }

    #[allow(clippy::too_many_lines)] // One auditable, no-retry HTTP transaction boundary.
    pub fn generate(&self, messages: &[Message]) -> Result<ProviderResponse> {
        self.generate_with_output_tokens(messages, self.config.output_tokens)
    }

    /// Generate one exact completion with a caller-selected immutable lane ceiling.
    ///
    /// The caller may select only a value already admitted by the root-owned
    /// steward configuration. The provider broker independently enforces its
    /// own ceiling, so this method cannot enlarge provider authority.
    #[allow(clippy::too_many_lines)] // One auditable, no-retry HTTP transaction boundary.
    pub fn generate_with_output_tokens(
        &self,
        messages: &[Message],
        output_tokens: u32,
    ) -> Result<ProviderResponse> {
        if !(64..=512).contains(&output_tokens)
            || ![
                self.config.output_tokens,
                self.config.source_authoring_output_tokens,
            ]
            .contains(&output_tokens)
        {
            return Err(Error::new(
                "provider output ceiling is not admitted for this steward lane",
            ));
        }
        let started = Instant::now();
        let mut stream = connect_provider(self.config, self.config.connect_timeout_ms)?;
        stream.set_nodelay()?;
        let request_body = if self.config.provider_broker.is_some() {
            serde_json::to_vec(&serde_json::json!({
                "model": self.config.model,
                "stream": false,
                "messages": messages,
                "max_tokens": output_tokens,
                "temperature": 0.35,
                "seed": 0
            }))?
        } else {
            serde_json::to_vec(&serde_json::json!({
                "model": self.config.model,
                "stream": false,
                "keep_alive": "2h",
                "messages": messages,
                "options": {
                    "num_ctx": self.config.context_tokens,
                    "num_predict": output_tokens,
                    "temperature": 0.35,
                    "seed": 0
                }
            }))?
        };
        if request_body.len() > 256 * 1024 {
            return Err(Error::new("provider request exceeds immutable bound"));
        }
        let request = provider_request_header(self.config, BROKER_INFERENCE_PATH, &request_body)?;
        let total_deadline = checked_deadline(
            started,
            self.config.total_timeout_ms,
            "provider total deadline overflow",
        )?;
        let startup_deadline = checked_deadline(
            Instant::now(),
            self.config.connect_timeout_ms,
            "provider startup deadline overflow",
        )?
        .min(total_deadline);
        write_request_startup(
            &mut stream,
            &[request.as_bytes(), &request_body],
            startup_deadline,
        )?;

        // Header latency begins only after the complete request has reached the
        // kernel. A peer that accepts but never reads is bounded separately by
        // `startup_deadline` and cannot consume the response-header budget.
        let header_deadline = checked_deadline(
            Instant::now(),
            self.config.header_timeout_ms,
            "provider header deadline overflow",
        )?
        .min(total_deadline);
        let mut wire = Vec::new();
        let header_end = loop {
            set_remaining_read_timeout(&stream, header_deadline)?;
            let mut block = [0_u8; 8 * 1024];
            let count = stream.read(&mut block)?;
            if count == 0 {
                return Err(Error::new(
                    "provider closed before complete response headers",
                ));
            }
            wire.extend_from_slice(&block[..count]);
            if wire.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
                return Err(Error::new("provider response exceeds immutable bound"));
            }
            if let Some(index) = find_bytes(&wire, b"\r\n\r\n") {
                break checked_index_add(index, 4, "provider header index overflow")?;
            }
            if wire.len() > MAX_HEADER_BYTES {
                return Err(Error::new(
                    "provider response headers exceed immutable bound",
                ));
            }
        };
        let header = std::str::from_utf8(&wire[..header_end])
            .map_err(|_| Error::new("provider response headers are not ASCII"))?;
        let parsed = parse_headers(header)?;
        if parsed.status != 200 {
            return Err(Error::new(format!(
                "provider returned HTTP {}",
                parsed.status
            )));
        }
        let mut body = wire[header_end..].to_vec();
        if let Some(length) = parsed.content_length {
            if length > MAX_BODY_BYTES {
                return Err(Error::new("provider body exceeds immutable bound"));
            }
            while body.len() < length {
                read_more(&stream, &mut body, total_deadline)?;
            }
            if body.len() != length {
                return Err(Error::new("provider sent bytes beyond Content-Length"));
            }
        } else if parsed.chunked {
            body = read_chunked(&stream, body, total_deadline)?;
        } else {
            while body.len() <= MAX_BODY_BYTES {
                set_remaining_read_timeout(&stream, total_deadline)?;
                let mut block = [0_u8; 8 * 1024];
                let count = stream.read(&mut block)?;
                if count == 0 {
                    break;
                }
                body.extend_from_slice(&block[..count]);
            }
            if body.len() > MAX_BODY_BYTES {
                return Err(Error::new("provider body exceeds immutable bound"));
            }
        }
        if self.config.provider_broker.is_some() {
            let response: OpenAiResponse = serde_json::from_slice(&body)?;
            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| Error::new("provider response omitted its first choice"))?;
            if choice.finish_reason.as_deref() != Some("stop")
                || choice.message.role != "assistant"
                || choice.message.content.is_empty()
            {
                return Err(Error::new(
                    "provider response is partial or not an assistant completion",
                ));
            }
            Ok(ProviderResponse {
                content: choice.message.content,
                prompt_tokens: response.usage.as_ref().map(|usage| usage.prompt_tokens),
                completion_tokens: response.usage.as_ref().map(|usage| usage.completion_tokens),
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            })
        } else {
            let response: OllamaResponse = serde_json::from_slice(&body)?;
            if !response.done
                || response.done_reason.as_deref() != Some("stop")
                || response.message.role != "assistant"
                || response.message.content.is_empty()
            {
                return Err(Error::new(
                    "provider response is partial or not an assistant completion",
                ));
            }
            Ok(ProviderResponse {
                content: response.message.content,
                prompt_tokens: response.prompt_eval_count,
                completion_tokens: response.eval_count,
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            })
        }
    }

    /// Issue exactly one bounded unload request on a fresh loopback connection.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, framing, schema, or non-confirmed unload.
    pub fn unload(&self) -> Result<UnloadResponse> {
        let started = Instant::now();
        let connect_ms = self.config.connect_timeout_ms.min(5_000);
        let header_ms = self.config.header_timeout_ms.min(30_000);
        let total_ms = self.config.total_timeout_ms.min(60_000).max(header_ms);
        let mut stream = connect_provider(self.config, connect_ms)?;
        stream.set_nodelay()?;
        let request_body = unload_request_body(self.config)?;
        let request_sha256 = crate::util::sha256(&request_body);
        let request = provider_request_header(self.config, BROKER_UNLOAD_PATH, &request_body)?;
        let total_deadline =
            checked_deadline(started, total_ms, "provider unload total deadline overflow")?;
        let startup_deadline = checked_deadline(
            Instant::now(),
            connect_ms,
            "provider unload startup deadline overflow",
        )?
        .min(total_deadline);
        write_request_startup(
            &mut stream,
            &[request.as_bytes(), &request_body],
            startup_deadline,
        )?;

        let header_deadline = checked_deadline(
            Instant::now(),
            header_ms,
            "provider unload header deadline overflow",
        )?
        .min(total_deadline);
        let mut wire = Vec::new();
        let header_end = loop {
            set_remaining_read_timeout(&stream, header_deadline)?;
            let mut block = [0_u8; 4 * 1024];
            let count = stream.read(&mut block)?;
            if count == 0 {
                return Err(Error::new("provider closed before unload headers"));
            }
            wire.extend_from_slice(&block[..count]);
            if let Some(index) = find_bytes(&wire, b"\r\n\r\n") {
                break checked_index_add(index, 4, "provider unload header index overflow")?;
            }
            if wire.len() > MAX_HEADER_BYTES {
                return Err(Error::new("provider unload headers exceed bound"));
            }
        };
        let body = read_unload_body(&stream, &wire, header_end, total_deadline)?;
        Ok(UnloadResponse {
            request_sha256,
            result_sha256: crate::util::sha256(&body),
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

fn connect_provider(config: &Config, connect_timeout_ms: u64) -> Result<ProviderStream> {
    if let Some(broker) = &config.provider_broker {
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        socket.connect_timeout(
            &SockAddr::unix(&broker.socket_path)?,
            Duration::from_millis(connect_timeout_ms),
        )?;
        let descriptor: std::os::fd::OwnedFd = socket.into();
        return Ok(ProviderStream::Unix(UnixStream::from(descriptor)));
    }
    let (host, port) = validate_loopback_origin(&config.ollama_origin)?;
    let ip = match host.as_str() {
        "127.0.0.1" => IpAddr::V4(Ipv4Addr::LOCALHOST),
        "::1" => IpAddr::V6(Ipv6Addr::LOCALHOST),
        _ => return Err(Error::new("provider host escaped loopback allowlist")),
    };
    Ok(ProviderStream::Tcp(TcpStream::connect_timeout(
        &SocketAddr::new(ip, port),
        Duration::from_millis(connect_timeout_ms),
    )?))
}

fn provider_request_header(config: &Config, broker_path: &str, body: &[u8]) -> Result<String> {
    if let Some(broker) = &config.provider_broker {
        let key = crate::util::read_stable_regular(&broker.request_key_path, 32)?;
        if crate::util::sha256(&key) != broker.request_key_sha256 {
            return Err(Error::new("provider broker credential identity changed"));
        }
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| Error::new("provider broker credential must contain exactly 32 bytes"))?;
        let nonce = provider_nonce()?;
        let authentication = provider_signature(&key, BROKER_CLIENT, broker_path, &nonce, body);
        return Ok(format!(
            "POST {broker_path} HTTP/1.1\r\nHost: {BROKER_AUTHORITY}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Provider-Client: {BROKER_CLIENT}\r\nX-Astrid-Provider-Nonce: {nonce}\r\nX-Astrid-Provider-Auth: {authentication}\r\n\r\n",
            body.len()
        ));
    }
    let (host, port) = validate_loopback_origin(&config.ollama_origin)?;
    let host_header = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let path = if broker_path == BROKER_INFERENCE_PATH {
        "/api/chat"
    } else if broker_path == BROKER_UNLOAD_PATH {
        "/api/generate"
    } else {
        return Err(Error::new("provider request path escaped fixed policy"));
    };
    Ok(format!(
        "POST {path} HTTP/1.1\r\nHost: {host_header}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    ))
}

fn provider_nonce() -> Result<String> {
    let milliseconds = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::new("system clock is before Unix epoch"))?
            .as_millis(),
    )
    .map_err(|_| Error::new("Unix milliseconds do not fit u64"))?;
    let entropy = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    Ok(format!("{milliseconds:016x}{}", &entropy[..48]))
}

fn provider_signature(
    key: &[u8; 32],
    client: &str,
    path: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    let digest = Sha256::digest(body);
    provider_hmac(
        key,
        BROKER_DOMAIN,
        &[
            client.as_bytes(),
            path.as_bytes(),
            nonce.as_bytes(),
            &digest,
        ],
    )
}

fn provider_hmac(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    normalized[..key.len()].copy_from_slice(key);
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut encoded = Vec::new();
    append_provider_field(&mut encoded, domain);
    for field in fields {
        append_provider_field(&mut encoded, field);
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(encoded);
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    format!("{:x}", outer.finalize())
}

fn append_provider_field(target: &mut Vec<u8>, field: &[u8]) {
    target.extend_from_slice(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(field);
}

fn read_unload_body(
    stream: &ProviderStream,
    wire: &[u8],
    header_end: usize,
    total_deadline: Instant,
) -> Result<Vec<u8>> {
    let header = std::str::from_utf8(&wire[..header_end])
        .map_err(|_| Error::new("provider unload headers are not ASCII"))?;
    let parsed = parse_headers(header)?;
    if parsed.status != 200 {
        return Err(Error::new(format!(
            "provider unload returned HTTP {}",
            parsed.status
        )));
    }
    let mut body = wire[header_end..].to_vec();
    if let Some(length) = parsed.content_length {
        if length > 64 * 1024 {
            return Err(Error::new("provider unload body exceeds bound"));
        }
        while body.len() < length {
            read_more(stream, &mut body, total_deadline)?;
        }
        if body.len() != length {
            return Err(Error::new(
                "provider unload exceeded declared Content-Length",
            ));
        }
    } else if parsed.chunked {
        body = read_chunked(stream, body, total_deadline)?;
        if body.len() > 64 * 1024 {
            return Err(Error::new("provider unload body exceeds bound"));
        }
    } else {
        return Err(Error::new(
            "provider unload requires explicit bounded response framing",
        ));
    }
    let confirmation: UnloadConfirmation = serde_json::from_slice(&body)?;
    if !confirmation.done || confirmation.done_reason.as_deref() != Some("unload") {
        return Err(Error::new("provider did not confirm model unload"));
    }
    Ok(body)
}

fn unload_request_body(config: &Config) -> Result<Vec<u8>> {
    if config.provider_broker.is_some() {
        crate::util::canonical_json(&serde_json::json!({
            "schema": "astrid.edge.provider_broker.unload.v1",
            "model": config.model
        }))
    } else {
        crate::util::canonical_json(&serde_json::json!({
            "model": config.model,
            "keep_alive": 0
        }))
    }
}

pub fn unload_request_sha256(config: &Config) -> Result<String> {
    Ok(crate::util::sha256(&unload_request_body(config)?))
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OllamaMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct UnloadConfirmation {
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
}

struct Headers {
    status: u16,
    content_length: Option<usize>,
    chunked: bool,
}

fn parse_headers(header: &str) -> Result<Headers> {
    let mut lines = header.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| Error::new("missing HTTP status"))?;
    let mut status_parts = status_line.split_whitespace();
    if status_parts.next() != Some("HTTP/1.1") {
        return Err(Error::new("provider did not use HTTP/1.1"));
    }
    let status = status_parts
        .next()
        .ok_or_else(|| Error::new("missing HTTP status code"))?
        .parse::<u16>()
        .map_err(|_| Error::new("invalid HTTP status code"))?;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new("malformed HTTP header"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "content-length" {
            if content_length.is_some() {
                return Err(Error::new("duplicate Content-Length rejected"));
            }
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| Error::new("invalid Content-Length"))?,
            );
        } else if name == "transfer-encoding" {
            chunked = value.eq_ignore_ascii_case("chunked");
            if !chunked {
                return Err(Error::new("unsupported Transfer-Encoding"));
            }
        } else if name == "content-encoding" && !value.eq_ignore_ascii_case("identity") {
            return Err(Error::new("compressed provider responses are rejected"));
        }
    }
    if content_length.is_some() && chunked {
        return Err(Error::new("ambiguous HTTP response framing rejected"));
    }
    Ok(Headers {
        status,
        content_length,
        chunked,
    })
}

fn read_more(stream: &ProviderStream, output: &mut Vec<u8>, deadline: Instant) -> Result<()> {
    set_remaining_read_timeout(stream, deadline)?;
    let mut block = [0_u8; 8 * 1024];
    let mut stream = stream;
    let count = stream.read(&mut block)?;
    if count == 0 {
        return Err(Error::new("provider response ended prematurely"));
    }
    output.extend_from_slice(&block[..count]);
    if output.len() > MAX_BODY_BYTES {
        return Err(Error::new("provider response exceeds immutable body bound"));
    }
    Ok(())
}

fn read_chunked(stream: &ProviderStream, mut wire: Vec<u8>, deadline: Instant) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut cursor = 0;
    loop {
        let line_end = loop {
            if let Some(relative) = find_bytes(&wire[cursor..], b"\r\n") {
                break checked_index_add(cursor, relative, "chunk line index overflow")?;
            }
            read_more(stream, &mut wire, deadline)?;
        };
        let size_text = std::str::from_utf8(&wire[cursor..line_end])
            .map_err(|_| Error::new("invalid chunk size"))?;
        if size_text.contains(';') || size_text.len() > 16 {
            return Err(Error::new("chunk extensions or oversized size rejected"));
        }
        let size =
            usize::from_str_radix(size_text, 16).map_err(|_| Error::new("invalid chunk size"))?;
        cursor = checked_index_add(line_end, 2, "chunk data index overflow")?;
        if size == 0 {
            let trailer_end = checked_index_add(cursor, 2, "chunk trailer index overflow")?;
            while wire.len() < trailer_end {
                read_more(stream, &mut wire, deadline)?;
            }
            if &wire[cursor..trailer_end] != b"\r\n" {
                return Err(Error::new("chunk trailer fields are rejected"));
            }
            return Ok(output);
        }
        if output.len().saturating_add(size) > MAX_BODY_BYTES {
            return Err(Error::new("chunked provider body exceeds immutable bound"));
        }
        while wire.len() < cursor.saturating_add(size).saturating_add(2) {
            read_more(stream, &mut wire, deadline)?;
        }
        let data_end = checked_index_add(cursor, size, "chunk payload index overflow")?;
        output.extend_from_slice(&wire[cursor..data_end]);
        cursor = data_end;
        let terminator_end = checked_index_add(cursor, 2, "chunk terminator index overflow")?;
        if &wire[cursor..terminator_end] != b"\r\n" {
            return Err(Error::new("malformed chunk terminator"));
        }
        cursor = terminator_end;
        if cursor > 64 * 1024 {
            wire.drain(..cursor);
            cursor = 0;
        }
    }
}

fn write_request_startup(
    stream: &mut ProviderStream,
    parts: &[&[u8]],
    deadline: Instant,
) -> Result<()> {
    set_remaining_write_timeout(stream, deadline)?;
    stream.set_nonblocking(true)?;
    let result = write_request_startup_nonblocking(stream, parts, deadline);
    let restore_result = stream.set_nonblocking(false);
    result?;
    restore_result?;
    Ok(())
}

fn checked_deadline(start: Instant, milliseconds: u64, error: &str) -> Result<Instant> {
    start
        .checked_add(Duration::from_millis(milliseconds))
        .ok_or_else(|| Error::new(error))
}

fn checked_index_add(left: usize, right: usize, error: &str) -> Result<usize> {
    left.checked_add(right).ok_or_else(|| Error::new(error))
}

fn write_request_startup_nonblocking(
    stream: &mut ProviderStream,
    parts: &[&[u8]],
    deadline: Instant,
) -> Result<()> {
    for part in parts {
        let mut written = 0_usize;
        while written < part.len() {
            ensure_startup_deadline(deadline)?;
            match stream.write(&part[written..]) {
                Ok(0) => return Err(Error::new("provider closed while request was starting")),
                Ok(count) => written = written.saturating_add(count),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {},
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    wait_for_startup_progress(deadline)?;
                },
                Err(error) => {
                    return Err(Error::new(format!(
                        "provider request startup failed: {error}"
                    )));
                },
            }
        }
    }
    loop {
        ensure_startup_deadline(deadline)?;
        match stream.flush() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {},
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                wait_for_startup_progress(deadline)?;
            },
            Err(error) => {
                return Err(Error::new(format!(
                    "provider request startup failed: {error}"
                )));
            },
        }
    }
}

fn ensure_startup_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Error::new("provider request startup deadline exceeded"));
    }
    Ok(())
}

fn wait_for_startup_progress(deadline: Instant) -> Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| Error::new("provider request startup deadline exceeded"))?;
    std::thread::sleep(remaining.min(Duration::from_millis(2)));
    Ok(())
}

fn remaining(deadline: Instant, phase: &str) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| Error::new(format!("provider {phase} deadline exceeded")))
}

fn set_remaining_read_timeout(stream: &ProviderStream, deadline: Instant) -> Result<()> {
    stream.set_read_timeout(Some(remaining(deadline, "response")?))?;
    Ok(())
}

fn set_remaining_write_timeout(stream: &ProviderStream, deadline: Instant) -> Result<()> {
    stream.set_write_timeout(Some(remaining(deadline, "request startup")?))?;
    Ok(())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use socket2::SockRef;

    use super::{ProviderStream, parse_headers, write_request_startup};

    #[test]
    fn ambiguous_or_compressed_framing_is_rejected() {
        assert!(
            parse_headers(
                "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .is_err()
        );
        assert!(parse_headers("HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\n").is_err());
    }

    #[test]
    fn request_startup_is_bounded_when_peer_accepts_but_does_not_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(0);
        let (release_sender, release_receiver) = mpsc::sync_channel(0);
        let peer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            SockRef::from(&stream).set_recv_buffer_size(1_024).unwrap();
            ready_sender.send(()).unwrap();
            release_receiver.recv().unwrap();
        });

        let stream = TcpStream::connect(address).unwrap();
        SockRef::from(&stream).set_send_buffer_size(1_024).unwrap();
        let mut stream = ProviderStream::Tcp(stream);
        ready_receiver.recv().unwrap();
        let request = vec![b'x'; 16 * 1024 * 1024];
        let started = Instant::now();
        let error = write_request_startup(
            &mut stream,
            &[&request],
            started + Duration::from_millis(50),
        )
        .unwrap_err();
        let elapsed = started.elapsed();

        assert_eq!(
            error.to_string(),
            "provider request startup deadline exceeded"
        );
        assert!(elapsed < Duration::from_millis(500), "elapsed={elapsed:?}");
        release_sender.send(()).unwrap();
        peer.join().unwrap();
    }
}
