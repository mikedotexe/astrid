use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};

use crate::config::parse_loopback_origin;
use crate::http::Operation;
use crate::{Config, Error, INFERENCE_PATH, Result};

const MAXIMUM_UPSTREAM_HEADER_BYTES: usize = 64 * 1024;
const MAXIMUM_CHUNK_LINE_BYTES: usize = 128;

pub struct UpstreamReceipt {
    pub status: u16,
    pub response_body_sha256: String,
    pub response_body_bytes: u64,
    pub elapsed_ms: u64,
}

pub fn transact(
    config: &Config,
    operation: Operation,
    body: &[u8],
    client: &mut impl Write,
    response_started: &mut bool,
) -> Result<UpstreamReceipt> {
    let (_, port) = parse_loopback_origin(&config.ollama_origin)?;
    let started = Instant::now();
    let total_deadline = started
        .checked_add(Duration::from_millis(config.total_timeout_ms))
        .ok_or_else(|| Error::new("upstream total deadline overflow"))?;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut upstream =
        TcpStream::connect_timeout(&address, Duration::from_millis(config.connect_timeout_ms))
            .map_err(|error| Error::new(format!("upstream connect failed: {error}")))?;
    upstream.set_nodelay(true)?;
    upstream.set_write_timeout(Some(Duration::from_millis(config.connect_timeout_ms)))?;
    let (path, upstream_body) = upstream_request(config, operation, body)?;
    let header = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        upstream_body.len()
    );
    upstream
        .write_all(header.as_bytes())
        .and_then(|()| upstream.write_all(&upstream_body))
        .and_then(|()| upstream.flush())
        .map_err(|error| Error::new(format!("upstream request write failed: {error}")))?;

    let header_deadline = Instant::now()
        .checked_add(Duration::from_millis(config.header_timeout_ms))
        .ok_or_else(|| Error::new("upstream header deadline overflow"))?
        .min(total_deadline);
    let (response_header, prefix) = read_response_header(&mut upstream, header_deadline)?;
    let parsed = parse_response_header(&response_header)?;
    let response_header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nX-Astrid-Provider-Gateway: immutable-v1\r\n\r\n",
        parsed.status,
        reason_phrase(parsed.status)
    );
    // From this point onward an error cannot be replaced with a second HTTP
    // response without corrupting the already-started client stream.
    *response_started = true;
    client.write_all(response_header.as_bytes())?;
    client.flush()?;

    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut deadline_reader = DeadlineReader {
        stream: &mut upstream,
        inter_chunk_timeout: Duration::from_millis(config.inter_chunk_timeout_ms),
        total_deadline,
    };
    match parsed.framing {
        Framing::ContentLength(length) => {
            if length > config.maximum_response_body_bytes {
                return Err(Error::new("upstream response exceeds immutable body bound"));
            }
            let reader = Cursor::new(prefix).chain(&mut deadline_reader);
            relay_exact(
                reader,
                length,
                client,
                &mut hasher,
                &mut count,
                config,
                total_deadline,
            )?;
        },
        Framing::Chunked => {
            let reader = BufReader::new(Cursor::new(prefix).chain(&mut deadline_reader));
            relay_chunked(
                reader,
                client,
                &mut hasher,
                &mut count,
                config,
                total_deadline,
            )?;
        },
        Framing::UntilClose => {
            let reader = Cursor::new(prefix).chain(&mut deadline_reader);
            relay_until_close(
                reader,
                client,
                &mut hasher,
                &mut count,
                config,
                total_deadline,
            )?;
        },
    }
    client.write_all(b"0\r\n\r\n")?;
    client.flush()?;
    Ok(UpstreamReceipt {
        status: parsed.status,
        response_body_sha256: format!("{:x}", hasher.finalize()),
        response_body_bytes: count,
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

struct DeadlineReader<'a> {
    stream: &'a mut TcpStream,
    inter_chunk_timeout: Duration,
    total_deadline: Instant,
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self
            .total_deadline
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "provider total response deadline expired",
            ));
        }
        self.stream
            .set_read_timeout(Some(remaining.min(self.inter_chunk_timeout)))?;
        self.stream.read(buffer)
    }
}

fn upstream_request(
    config: &Config,
    operation: Operation,
    body: &[u8],
) -> Result<(&'static str, Vec<u8>)> {
    match operation {
        Operation::Inference => Ok((INFERENCE_PATH, body.to_vec())),
        Operation::Warmup => Ok((
            "/api/generate",
            serde_json::to_vec(&serde_json::json!({
                "model": config.model,
                "prompt": "Reply with exactly OK and no other text.",
                "stream": false,
                "think": false,
                "keep_alive": config.keep_alive,
                "options": {
                    "num_ctx": 512,
                    "num_predict": 2,
                    "seed": 0,
                    "temperature": 0,
                },
            }))?,
        )),
        Operation::Unload => Ok((
            "/api/generate",
            serde_json::to_vec(&serde_json::json!({
                "model": config.model,
                "prompt": "",
                "stream": false,
                "keep_alive": 0,
            }))?,
        )),
    }
}

fn read_response_header(stream: &mut TcpStream, deadline: Instant) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut wire = Vec::new();
    loop {
        set_read_deadline(stream, deadline)?;
        let mut block = [0_u8; 4096];
        let count = stream
            .read(&mut block)
            .map_err(|error| Error::new(format!("upstream header read failed: {error}")))?;
        if count == 0 {
            return Err(Error::new("upstream closed before response headers"));
        }
        wire.extend_from_slice(&block[..count]);
        if let Some(position) = wire.windows(4).position(|value| value == b"\r\n\r\n") {
            let end = position.saturating_add(4);
            if end > MAXIMUM_UPSTREAM_HEADER_BYTES {
                return Err(Error::new(
                    "upstream response headers exceed immutable bound",
                ));
            }
            return Ok((wire[..end].to_vec(), wire[end..].to_vec()));
        }
        if wire.len() > MAXIMUM_UPSTREAM_HEADER_BYTES {
            return Err(Error::new(
                "upstream response headers exceed immutable bound",
            ));
        }
    }
}

enum Framing {
    ContentLength(u64),
    Chunked,
    UntilClose,
}

struct ParsedResponse {
    status: u16,
    framing: Framing,
}

fn parse_response_header(header: &[u8]) -> Result<ParsedResponse> {
    let value = std::str::from_utf8(header)
        .map_err(|_| Error::new("upstream response headers are not ASCII"))?;
    if !value.is_ascii() {
        return Err(Error::new("upstream response headers are not ASCII"));
    }
    let mut lines = value.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| Error::new("upstream response status is absent"))?;
    let mut status_parts = status_line.split_whitespace();
    if status_parts.next() != Some("HTTP/1.1") {
        return Err(Error::new("upstream response protocol is not HTTP/1.1"));
    }
    let status = status_parts
        .next()
        .ok_or_else(|| Error::new("upstream response status is absent"))?
        .parse::<u16>()
        .map_err(|_| Error::new("upstream response status is invalid"))?;
    if !(200..=599).contains(&status) || (300..=399).contains(&status) {
        return Err(Error::new(
            "upstream redirect or invalid status is rejected",
        ));
    }
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        if line.starts_with([' ', '\t']) {
            return Err(Error::new("upstream folded headers are rejected"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new("upstream response header is malformed"))?;
        let name = name.to_ascii_lowercase();
        if headers.insert(name, value.trim()).is_some() {
            return Err(Error::new("upstream duplicate response header is rejected"));
        }
    }
    let framing = match (
        headers.get("content-length"),
        headers.get("transfer-encoding"),
    ) {
        (Some(_), Some(_)) => return Err(Error::new("upstream response framing is ambiguous")),
        (Some(raw), None) => {
            let length = raw
                .parse::<u64>()
                .map_err(|_| Error::new("upstream Content-Length is invalid"))?;
            if length.to_string() != *raw {
                return Err(Error::new("upstream Content-Length is not canonical"));
            }
            Framing::ContentLength(length)
        },
        (None, Some(value)) if value.eq_ignore_ascii_case("chunked") => Framing::Chunked,
        (None, Some(_)) => return Err(Error::new("upstream transfer encoding is unsupported")),
        (None, None) => Framing::UntilClose,
    };
    Ok(ParsedResponse { status, framing })
}

fn relay_exact(
    mut source: impl Read,
    length: u64,
    client: &mut impl Write,
    hasher: &mut Sha256,
    count: &mut u64,
    config: &Config,
    deadline: Instant,
) -> Result<()> {
    let mut remaining = length;
    let mut block = [0_u8; 8192];
    while remaining > 0 {
        ensure_deadline(deadline)?;
        let maximum =
            usize::try_from(remaining.min(u64::try_from(block.len()).unwrap_or(u64::MAX)))
                .unwrap_or(block.len());
        let read = source.read(&mut block[..maximum])?;
        if read == 0 {
            return Err(Error::new("upstream body ended before Content-Length"));
        }
        relay_block(&block[..read], client, hasher, count, config)?;
        remaining = remaining.saturating_sub(u64::try_from(read).unwrap_or(u64::MAX));
    }
    Ok(())
}

fn relay_until_close(
    mut source: impl Read,
    client: &mut impl Write,
    hasher: &mut Sha256,
    count: &mut u64,
    config: &Config,
    deadline: Instant,
) -> Result<()> {
    let mut block = [0_u8; 8192];
    loop {
        ensure_deadline(deadline)?;
        let read = source.read(&mut block)?;
        if read == 0 {
            return Ok(());
        }
        relay_block(&block[..read], client, hasher, count, config)?;
    }
}

fn relay_chunked(
    mut source: impl BufRead,
    client: &mut impl Write,
    hasher: &mut Sha256,
    count: &mut u64,
    config: &Config,
    deadline: Instant,
) -> Result<()> {
    loop {
        ensure_deadline(deadline)?;
        let mut line = String::new();
        let read = source.read_line(&mut line)?;
        if read == 0 || read > MAXIMUM_CHUNK_LINE_BYTES || !line.ends_with("\r\n") {
            return Err(Error::new("upstream chunk header is malformed"));
        }
        let raw = line.trim_end_matches("\r\n");
        let size_token = raw.split_once(';').map_or(raw, |(size, _)| size);
        let size = u64::from_str_radix(size_token, 16)
            .map_err(|_| Error::new("upstream chunk length is invalid"))?;
        if size == 0 {
            line.clear();
            let trailer = source.read_line(&mut line)?;
            if trailer == 0 || trailer > MAXIMUM_CHUNK_LINE_BYTES {
                return Err(Error::new("upstream chunk trailer is malformed"));
            }
            if line == "\r\n" {
                return Ok(());
            }
            return Err(Error::new("upstream response trailers are rejected"));
        }
        if size > config.maximum_response_body_bytes.saturating_sub(*count) {
            return Err(Error::new("upstream response exceeds immutable body bound"));
        }
        relay_exact(
            source.by_ref(),
            size,
            client,
            hasher,
            count,
            config,
            deadline,
        )?;
        let mut terminator = [0_u8; 2];
        source.read_exact(&mut terminator)?;
        if terminator != *b"\r\n" {
            return Err(Error::new("upstream chunk terminator is malformed"));
        }
    }
}

fn relay_block(
    block: &[u8],
    client: &mut impl Write,
    hasher: &mut Sha256,
    count: &mut u64,
    config: &Config,
) -> Result<()> {
    let next = count.saturating_add(u64::try_from(block.len()).unwrap_or(u64::MAX));
    if next > config.maximum_response_body_bytes {
        return Err(Error::new("upstream response exceeds immutable body bound"));
    }
    write!(client, "{:x}\r\n", block.len())?;
    client.write_all(block)?;
    client.write_all(b"\r\n")?;
    client.flush()?;
    hasher.update(block);
    *count = next;
    Ok(())
}

fn set_read_deadline(stream: &TcpStream, deadline: Instant) -> Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Error::new("upstream response header deadline expired"));
    }
    stream.set_read_timeout(Some(remaining))?;
    Ok(())
}

fn ensure_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Error::new("upstream total response deadline expired"));
    }
    Ok(())
}

const fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Upstream Status",
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use super::{parse_response_header, transact};
    use crate::http::Operation;

    #[test]
    fn response_parser_rejects_redirects_and_ambiguous_framing() {
        assert!(parse_response_header(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n").is_ok());
        assert!(parse_response_header(b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\n\r\n").is_err());
        assert!(
            parse_response_header(
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .is_err()
        );
    }

    #[test]
    fn exact_inference_reaches_only_chat_completions_and_is_relayed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            assert!(request.starts_with(b"POST /v1/chat/completions HTTP/1.1\r\n"));
            assert!(!request.windows(11).any(|window| window == b"/api/delete"));
            let response = br#"{"choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                response.len()
            )
            .unwrap();
            stream.write_all(response).unwrap();
        });
        let (mut config, _temporary) = crate::config::tests_support::config_for_protocol_tests();
        config.ollama_origin = format!("http://127.0.0.1:{port}");
        let body = serde_json::to_vec(&serde_json::json!({
            "model":"qwen3.5:4b",
            "messages":[{"role":"user","content":"hi"}],
            "stream":false,
            "max_tokens":64,
            "keep_alive":"2h",
            "options":{"num_ctx":4096}
        }))
        .unwrap();
        let mut client = Vec::new();
        let mut response_started = false;
        let receipt = transact(
            &config,
            Operation::Inference,
            &body,
            &mut client,
            &mut response_started,
        )
        .unwrap();
        assert_eq!(receipt.status, 200);
        assert!(response_started);
        assert!(client.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(client.windows(2).any(|window| window == b"ok"));
        worker.join().unwrap();
    }

    #[test]
    fn total_deadline_caps_each_post_header_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_http_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")
                .unwrap();
            std::thread::sleep(Duration::from_millis(250));
            let _ = stream.write_all(b"late");
        });
        let (mut config, _temporary) = crate::config::tests_support::config_for_protocol_tests();
        config.ollama_origin = format!("http://127.0.0.1:{port}");
        config.header_timeout_ms = 100;
        config.inter_chunk_timeout_ms = 1_000;
        config.total_timeout_ms = 80;
        let started = Instant::now();
        let mut client = Vec::new();
        let mut response_started = false;
        assert!(
            transact(
                &config,
                Operation::Inference,
                br#"{"model":"qwen3.5:4b"}"#,
                &mut client,
                &mut response_started,
            )
            .is_err()
        );
        assert!(response_started);
        assert!(started.elapsed() < Duration::from_millis(220));
        worker.join().unwrap();
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
        let mut wire = Vec::new();
        let header_end = loop {
            let mut byte = [0_u8; 1];
            stream.read_exact(&mut byte).unwrap();
            wire.push(byte[0]);
            if wire.ends_with(b"\r\n\r\n") {
                break wire.len();
            }
        };
        let header = std::str::from_utf8(&wire[..header_end]).unwrap();
        let length = header
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap();
        let mut body = vec![0_u8; length];
        stream.read_exact(&mut body).unwrap();
        wire.extend_from_slice(&body);
        wire
    }
}
