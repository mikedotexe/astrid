use std::collections::BTreeMap;
use std::io::Read;

use crate::auth::{
    AUTH_HEADER, CLIENT_HEADER, NONCE_HEADER, ReplayGuard, request_hash, request_signature,
    verify_hmac,
};
use crate::config::{BROKER_HTTP_AUTHORITY, LISTEN_PATH};
use crate::{Config, Error, FETCH_PATH, FetchRequest, Result, SearchRequest};

const MAXIMUM_HEADER_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug)]
pub enum BrokerRequest {
    Search(SearchRequest),
    Fetch(FetchRequest),
}

#[derive(Clone, Debug)]
pub struct AuthenticatedBrokerRequest {
    pub request: BrokerRequest,
    pub client_id: String,
    pub nonce: String,
    pub request_hash: String,
}

pub fn read_broker_request(
    reader: &mut impl Read,
    config: &Config,
    request_key: &[u8; 32],
    replay: &ReplayGuard,
) -> Result<AuthenticatedBrokerRequest> {
    let mut wire = Vec::new();
    let header_end = loop {
        let mut block = [0_u8; 1024];
        let count = reader.read(&mut block)?;
        if count == 0 {
            return Err(Error::new("request ended before complete headers"));
        }
        wire.extend_from_slice(&block[..count]);
        if let Some(position) = wire.windows(4).position(|value| value == b"\r\n\r\n") {
            break position.saturating_add(4);
        }
        if wire.len() > MAXIMUM_HEADER_BYTES {
            return Err(Error::new("request headers exceed immutable bound"));
        }
    };
    if header_end > MAXIMUM_HEADER_BYTES {
        return Err(Error::new("request headers exceed immutable bound"));
    }
    let header = std::str::from_utf8(&wire[..header_end])
        .map_err(|_| Error::new("request headers are not valid ASCII-compatible text"))?;
    if !header.is_ascii() {
        return Err(Error::new("request headers are not ASCII"));
    }
    let (path, host, length, client_id, nonce, supplied_auth) = validate_headers(header, config)?;
    if length > usize::try_from(config.maximum_request_body_bytes).unwrap_or(usize::MAX) {
        return Err(Error::new("request body exceeds immutable bound"));
    }
    let mut body = wire[header_end..].to_vec();
    if body.len() > length {
        return Err(Error::new(
            "request pipelining or excess framing is rejected",
        ));
    }
    while body.len() < length {
        let remaining = length.saturating_sub(body.len()).min(1024);
        let mut block = [0_u8; 1024];
        let count = reader.read(&mut block[..remaining])?;
        if count == 0 {
            return Err(Error::new("request body ended before Content-Length"));
        }
        body.extend_from_slice(&block[..count]);
    }
    if client_id != config.client_id {
        return Err(Error::new(
            "request client identity does not match this Unix listener",
        ));
    }
    let expected_auth = request_signature(request_key, &client_id, &path, &host, &nonce, &body)?;
    if !verify_hmac(&expected_auth, &supplied_auth) {
        return Err(Error::new("request authentication failed"));
    }
    replay.accept(&client_id, &nonce)?;
    let request_hash = request_hash(&client_id, &path, &host, &nonce, &body)?;
    let request = match path.as_str() {
        LISTEN_PATH => {
            let request: SearchRequest = serde_json::from_slice(&body)?;
            request.validate(config)?;
            BrokerRequest::Search(request)
        },
        FETCH_PATH => {
            let request: FetchRequest = serde_json::from_slice(&body)?;
            request.validate()?;
            BrokerRequest::Fetch(request)
        },
        _ => return Err(Error::new("request path is not allowlisted")),
    };
    Ok(AuthenticatedBrokerRequest {
        request,
        client_id,
        nonce,
        request_hash,
    })
}

#[cfg(test)]
pub fn read_search_request(
    reader: &mut impl Read,
    config: &Config,
    request_key: &[u8; 32],
    replay: &ReplayGuard,
) -> Result<SearchRequest> {
    match read_broker_request(reader, config, request_key, replay)?.request {
        BrokerRequest::Search(request) => Ok(request),
        BrokerRequest::Fetch(_) => Err(Error::new("request is not a search request")),
    }
}

fn validate_headers(
    header: &str,
    config: &Config,
) -> Result<(String, String, usize, String, String, String)> {
    let mut lines = header.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| Error::new("request line is missing"))?;
    let path = match request_line {
        value if value == format!("POST {LISTEN_PATH} HTTP/1.1") => LISTEN_PATH,
        value if value == format!("POST {FETCH_PATH} HTTP/1.1") => FETCH_PATH,
        _ => return Err(Error::new("request method, path, or protocol is not exact")),
    };
    if path.is_empty() {
        return Err(Error::new("request method, path, or protocol is not exact"));
    }
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        if line.starts_with([' ', '\t']) {
            return Err(Error::new("folded request headers are rejected"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new("request header is malformed"))?;
        let name = name.to_ascii_lowercase();
        if !matches!(
            name.as_str(),
            "host"
                | "content-type"
                | "accept"
                | "connection"
                | "content-length"
                | CLIENT_HEADER
                | NONCE_HEADER
                | AUTH_HEADER
        ) || name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || headers.insert(name, value.trim()).is_some()
        {
            return Err(Error::new(
                "request headers are duplicated or not allowlisted",
            ));
        }
    }
    let client_id = headers
        .get(CLIENT_HEADER)
        .ok_or_else(|| Error::new("request omitted exact client identity"))?
        .to_string();
    if client_id != config.client_id {
        return Err(Error::new(
            "request client identity does not match this Unix listener",
        ));
    }
    if headers.get("host").copied() != Some(BROKER_HTTP_AUTHORITY)
        || headers.get("content-type").copied() != Some("application/json")
        || headers.get("accept").copied() != Some("application/json")
        || headers.get("connection").copied() != Some("close")
    {
        return Err(Error::new(
            "request headers do not match exact broker contract",
        ));
    }
    let nonce = headers
        .get(NONCE_HEADER)
        .ok_or_else(|| Error::new("request omitted authentication nonce"))?
        .to_string();
    let auth = headers
        .get(AUTH_HEADER)
        .ok_or_else(|| Error::new("request omitted authentication signature"))?
        .to_string();
    let raw_length = headers
        .get("content-length")
        .ok_or_else(|| Error::new("request omitted Content-Length"))?;
    let length = raw_length
        .parse::<usize>()
        .map_err(|_| Error::new("request Content-Length is invalid"))?;
    if length == 0 || length.to_string() != *raw_length {
        return Err(Error::new("request Content-Length is not canonical"));
    }
    Ok((
        path.to_owned(),
        BROKER_HTTP_AUTHORITY.to_owned(),
        length,
        client_id,
        nonce,
        auth,
    ))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{BrokerRequest, read_broker_request, read_search_request};
    use crate::auth::{RUNTIME_CLIENT_ID, ReplayGuard, request_signature};
    use crate::{Config, FETCH_REQUEST_SCHEMA};

    fn config() -> Config {
        Config::from_json(
            br#"{"schema":"astrid.edge.web_broker.config.v3","client_id":"edge-runtime","socket_path":"/run/astrid-edge-self-change/web-runtime.sock","expected_peer_uid":1001,"socket_gid":1003,"upstream_origin":"https://search.brave.com/search","connect_timeout_ms":2000,"header_timeout_ms":8000,"total_timeout_ms":20000,"client_read_timeout_ms":2000,"client_write_timeout_ms":2000,"maximum_request_body_bytes":4096,"maximum_upstream_body_bytes":1048576,"maximum_results":5,"maximum_concurrent_requests":4,"maximum_searches_per_hour":8,"maximum_searches_per_utc_day":24,"quota_state_path":"/var/lib/astrid-edge-web-runtime/search-quota.jsonl","request_key_path":"/run/credentials/astrid-edge-web-broker-runtime.service/request.key","request_key_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","response_signing_key_path":"/run/credentials/astrid-edge-web-broker-runtime.service/response-signing.key","response_signing_key_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","response_verify_key_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#,
        )
        .unwrap()
    }

    fn request(body: &[u8]) -> Vec<u8> {
        request_for("/v1/search", body)
    }

    fn request_for(path: &str, body: &[u8]) -> Vec<u8> {
        let nonce = format!("{:016x}{}", now_millis(), "a".repeat(48));
        let auth = request_signature(
            &[0x42; 32],
            RUNTIME_CLIENT_ID,
            path,
            "astrid-edge-web-broker",
            &nonce,
            body,
        )
        .unwrap();
        format!(
            "POST {path} HTTP/1.1\r\nHost: astrid-edge-web-broker\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\nX-Astrid-Web-Client: {RUNTIME_CLIENT_ID}\r\nX-Astrid-Web-Nonce: {nonce}\r\nX-Astrid-Web-Auth: {auth}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    #[test]
    fn exact_framed_request_decodes() {
        let body = br#"{"schema":"astrid.edge.web_search.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","query":"reservoir entropy","limit":3}"#;
        let parsed = read_search_request(
            &mut Cursor::new(request(body)),
            &config(),
            &[0x42; 32],
            &ReplayGuard::default(),
        )
        .unwrap();
        assert_eq!(parsed.query, "reservoir entropy");
        assert_eq!(parsed.limit, 3);
    }

    #[test]
    fn malformed_ambiguous_and_excess_framing_are_rejected() {
        let body = br#"{"schema":"astrid.edge.web_search.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","query":"state entropy","limit":1}"#;
        let mut duplicate = request(body);
        let marker = duplicate
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .unwrap();
        duplicate.splice(marker..marker, b"Content-Length: 1\r\n".iter().copied());
        assert!(
            read_search_request(
                &mut Cursor::new(duplicate),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );

        let mut pipelined = request(body);
        pipelined.extend_from_slice(b"GET / HTTP/1.1\r\n\r\n");
        assert!(
            read_search_request(
                &mut Cursor::new(pipelined),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );

        let mut chunked = request(body);
        let marker = chunked
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .unwrap();
        chunked.splice(
            marker..marker,
            b"Transfer-Encoding: chunked\r\n".iter().copied(),
        );
        assert!(
            read_search_request(
                &mut Cursor::new(chunked),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_body_is_rejected_before_json_parsing() {
        let wire = b"POST /v1/search HTTP/1.1\r\nHost: astrid-edge-web-broker\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: 4097\r\n\r\n";
        assert!(
            read_search_request(
                &mut Cursor::new(wire),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn fetch_requires_the_fetch_path_and_schema_together() {
        let body = serde_json::to_vec(&serde_json::json!({
            "schema": FETCH_REQUEST_SCHEMA,
            "trace_id": "11111111-1111-4111-8111-111111111111",
            "url": "https://example.org/paper",
            "max_chars": 8_000
        }))
        .unwrap();
        let parsed = read_broker_request(
            &mut Cursor::new(request_for("/v1/fetch", &body)),
            &config(),
            &[0x42; 32],
            &ReplayGuard::default(),
        )
        .unwrap();
        assert!(matches!(parsed.request, BrokerRequest::Fetch(_)));
        assert!(
            read_broker_request(
                &mut Cursor::new(request(&body)),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn authenticated_request_replay_and_tampering_are_rejected() {
        let body = br#"{"schema":"astrid.edge.web_search.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","query":"state entropy","limit":1}"#;
        let wire = request(body);
        let replay = ReplayGuard::default();
        read_search_request(
            &mut Cursor::new(wire.clone()),
            &config(),
            &[0x42; 32],
            &replay,
        )
        .unwrap();
        assert!(
            read_search_request(
                &mut Cursor::new(wire.clone()),
                &config(),
                &[0x42; 32],
                &replay,
            )
            .is_err()
        );
        let mut tampered = wire;
        *tampered.last_mut().unwrap() ^= 1;
        assert!(
            read_search_request(
                &mut Cursor::new(tampered),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn one_client_cannot_impersonate_the_other() {
        let body = br#"{"schema":"astrid.edge.web_search.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","query":"state entropy","limit":1}"#;
        let wire = String::from_utf8(request(body))
            .unwrap()
            .replace(RUNTIME_CLIENT_ID, "edge-steward")
            .into_bytes();
        assert!(
            read_search_request(
                &mut Cursor::new(wire),
                &config(),
                &[0x42; 32],
                &ReplayGuard::default(),
            )
            .is_err()
        );
    }

    fn now_millis() -> u64 {
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    }
}
