use astrid_guest::{capsule_result, http, ipc, serde_json, tool};
use serde_json::Value;
use std::collections::BTreeSet;

struct HttpCapsule;

impl astrid_guest::Guest for HttpCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_fetch_url" => handle_tool(&payload, fetch_url),
            "tool_execute_search_web" => handle_tool(&payload, search_web),
            "tool_describe" => describe(),
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn handle_tool(
    payload: &[u8],
    handler: fn(&Value) -> Result<String, String>,
) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(err) => return capsule_result::deny(err),
    };
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(err) => tool::publish_error(&request.call_id, &request.tool_name, err),
    }
}

fn fetch_url(args: &Value) -> Result<String, String> {
    let url = tool::required_string_arg(args, "url")?;
    let method = tool::string_arg(args, "method")
        .unwrap_or_else(|| "GET".to_string())
        .to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "HEAD") {
        return Err("fetch_url is read-only; method must be GET or HEAD".to_string());
    }
    let headers = args
        .get("headers")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let response = http::request(&method, &url, headers, None)?;
    let original_body_bytes = response.body.len();
    let requested_max = tool::u64_arg(args, "max_chars").unwrap_or(16_000);
    let max_chars = usize::try_from(requested_max.min(32_000)).unwrap_or(32_000);
    let mut body = String::from_utf8_lossy(&response.body).into_owned();
    let truncated = body.chars().count() > max_chars;
    if truncated {
        body = body.chars().take(max_chars).collect();
    }
    let headers = response
        .headers
        .into_iter()
        .map(|header| serde_json::json!({"key": header.key, "value": header.value}))
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "url": url,
        "status": response.status,
        "headers": headers,
        "body": body,
        "original_body_bytes": original_body_bytes,
        "truncated": truncated,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn search_web(args: &Value) -> Result<String, String> {
    let query = tool::required_string_arg(args, "query")?;
    let query = query.trim();
    if query.is_empty() {
        return Err("search_web query must not be empty".to_string());
    }
    if query.chars().count() > 300 {
        return Err("search_web query must not exceed 300 characters".to_string());
    }
    let requested_count = tool::u64_arg(args, "count").unwrap_or(5).clamp(1, 8);
    let count = usize::try_from(requested_count).unwrap_or(5);
    let search_url = format!(
        "https://search.brave.com/search?q={}&source=web",
        encode_query(query)
    );
    let headers = vec![
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (compatible; AstridEdge/0.1; read-only search)".to_string(),
        ),
        (
            "Accept".to_string(),
            "text/html,application/xhtml+xml".to_string(),
        ),
    ];
    let response = http::request("GET", &search_url, headers, None)?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "search_web provider returned HTTP {}",
            response.status
        ));
    }
    let body = String::from_utf8_lossy(&response.body);
    let broker_verified = response.headers.iter().any(|header| {
        header
            .key
            .eq_ignore_ascii_case("x-astrid-immutable-web-broker")
            && header.value == "v1"
    });
    let results = if broker_verified {
        parse_immutable_broker_results(&body, count)?
    } else {
        parse_brave_results(&body, count)
    };
    if results.is_empty() {
        return Err(
            "search_web provider returned no parseable results; retry later or fetch a known URL"
                .to_string(),
        );
    }
    let payload = serde_json::json!({
        "schema": "astrid_search_web_results_v1",
        "query": query,
        "provider": if broker_verified {
            "immutable_cpu_edge_web_broker"
        } else {
            "brave_public_html"
        },
        "result_count": results.len(),
        "results": results,
        "authority": "read_only_public_web_search",
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn parse_immutable_broker_results(body: &str, count: usize) -> Result<Vec<Value>, String> {
    let payload: Value = serde_json::from_str(body)
        .map_err(|_| "immutable web broker returned malformed JSON".to_string())?;
    if payload.get("schema").and_then(Value::as_str) != Some("astrid.edge.web_search.response.v1") {
        return Err("immutable web broker returned an unsupported schema".to_string());
    }
    let results = payload
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "immutable web broker omitted search results".to_string())?;
    if results.len() > count || results.len() > 5 {
        return Err("immutable web broker exceeded the requested result count".to_string());
    }
    let mut projected = Vec::with_capacity(results.len());
    for (index, result) in results.iter().enumerate() {
        let object = result
            .as_object()
            .ok_or_else(|| "immutable web broker result is not an object".to_string())?;
        if !object
            .keys()
            .all(|key| matches!(key.as_str(), "title" | "url" | "snippet"))
            || object.len() != 3
        {
            return Err("immutable web broker result escaped its exact schema".to_string());
        }
        let title = bounded_broker_field(object.get("title"), 300, "title")?;
        let url = bounded_broker_field(object.get("url"), 2_048, "URL")?;
        let snippet = bounded_broker_field(object.get("snippet"), 700, "snippet")?;
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("immutable web broker result URL is not public HTTP".to_string());
        }
        projected.push(serde_json::json!({
            "position": index.saturating_add(1),
            "title": title,
            "url": url,
            "snippet": snippet,
        }));
    }
    Ok(projected)
}

fn bounded_broker_field(
    value: Option<&Value>,
    maximum: usize,
    label: &str,
) -> Result<String, String> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| format!("immutable web broker result omitted {label}"))?;
    if value.is_empty()
        || value.chars().count() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(format!(
            "immutable web broker result {label} exceeds bounds"
        ));
    }
    Ok(value.to_string())
}

fn parse_brave_results(body: &str, count: usize) -> Vec<Value> {
    let mut results = Vec::new();
    let mut seen_urls = BTreeSet::new();
    for chunk in body.split("<div class=\"snippet").skip(1) {
        if results.len() >= count {
            break;
        }
        let Some(url) = result_url(chunk) else {
            continue;
        };
        let url = decode_html_entities(url);
        if url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .is_none()
            || !seen_urls.insert(url.clone())
        {
            continue;
        }
        let title = element_text(chunk, "title search-snippet-title").unwrap_or_else(|| {
            url.split('/')
                .nth(2)
                .filter(|host| !host.is_empty())
                .unwrap_or("Untitled result")
                .to_string()
        });
        let snippet = element_text(chunk, "content desktop-default-regular").unwrap_or_default();
        results.push(serde_json::json!({
            "position": results.len().saturating_add(1),
            "title": title,
            "url": url,
            "snippet": snippet.chars().take(700).collect::<String>(),
        }));
    }
    results
}

fn result_url(chunk: &str) -> Option<&str> {
    for anchor in chunk.split("<a ").skip(1) {
        let Some(tag_end) = anchor.find('>') else {
            continue;
        };
        let attributes = &anchor[..tag_end];
        let class = attribute(attributes, "class").unwrap_or_default();
        if class.split_whitespace().any(|value| value == "l1") {
            return attribute(attributes, "href");
        }
    }
    None
}

fn element_text(chunk: &str, class_fragment: &str) -> Option<String> {
    let class_position = chunk.find(class_fragment)?;
    let content_start = class_position.checked_add(chunk[class_position..].find('>')?)?;
    let content_start = content_start.checked_add(1)?;
    let content = chunk.get(content_start..)?;
    let content_end = content.find("</div>")?;
    let text = strip_html(content.get(..content_end)?);
    (!text.is_empty()).then_some(text)
}

fn attribute<'a>(attributes: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("{name}=\"");
    let start = attributes.find(&marker)?.checked_add(marker.len())?;
    let value = attributes.get(start..)?;
    let end = value.find('"')?;
    value.get(..end)
}

fn strip_html(value: &str) -> String {
    let mut text = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => {
                in_tag = true;
                if !text.ends_with(char::is_whitespace) {
                    text.push(' ');
                }
            },
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {},
        }
    }
    let normalized = decode_html_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    [
        (".", "."),
        (",", ","),
        (";", ";"),
        (":", ":"),
        ("?", "?"),
        ("!", "!"),
    ]
    .into_iter()
    .fold(normalized, |text, (punctuation, replacement)| {
        text.replace(&format!(" {punctuation}"), replacement)
    })
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn encode_query(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            },
            b' ' => encoded.push('+'),
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                encoded.push('%');
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            },
        }
    }
    encoded
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = serde_json::json!({
        "capsule": "astrid-capsule-http",
        "tools": [
            {
                "name": "fetch_url",
                "description": "Fetch a public web URL with a read-only GET or HEAD request. Private, loopback, link-local, and metadata-network addresses are blocked. Response text is capped for the local model context.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "Absolute public http:// or https:// URL to fetch."
                        },
                        "method": {
                            "type": "string",
                            "enum": ["GET", "HEAD"],
                            "default": "GET"
                        },
                        "headers": {
                            "type": "object",
                            "description": "Optional string-valued request headers.",
                            "additionalProperties": {"type": "string"}
                        },
                        "max_chars": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 32000,
                            "default": 16000,
                            "description": "Maximum response-body characters to return."
                        }
                    },
                    "required": ["url"],
                    "additionalProperties": false
                }
            },
            {
                "name": "search_web",
                "description": "Call this when the user asks to search or browse the web. It searches the public web by query and returns bounded result titles, snippets, and URLs. This is read-only, uses a fixed public search origin, and does not access private or local networks. Do not claim a search happened until this tool returns successfully.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 300,
                            "description": "Specific public-web search query."
                        },
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 8,
                            "default": 5,
                            "description": "Maximum number of results to return."
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            }
        ]
    });
    if let Err(err) = ipc::publish_json("tool.v1.response.describe.astrid-capsule-http", &payload) {
        return capsule_result::deny(err);
    }
    capsule_result::continue_json(&payload)
}

astrid_guest::export!(HttpCapsule);

#[cfg(test)]
mod broker_tests {
    use astrid_guest::serde_json;

    use super::parse_immutable_broker_results;

    #[test]
    fn immutable_broker_results_are_exact_bounded_and_positioned() {
        let body = serde_json::json!({
            "schema": "astrid.edge.web_search.response.v1",
            "results": [{
                "title": "Reservoir study",
                "url": "https://example.com/study",
                "snippet": "A bounded abstract."
            }]
        })
        .to_string();
        let results = parse_immutable_broker_results(&body, 5).unwrap();
        assert_eq!(results[0]["position"], 1);
        assert_eq!(results[0]["title"], "Reservoir study");

        let mut injected: serde_json::Value = serde_json::from_str(&body).unwrap();
        injected["results"][0]["headers"] = serde_json::json!({"secret": "value"});
        assert!(parse_immutable_broker_results(&injected.to_string(), 5).is_err());
    }

    #[test]
    fn immutable_broker_results_reject_wrong_schema_count_and_private_shapes() {
        let wrong = serde_json::json!({
            "schema": "astrid.edge.web_search.response.future",
            "results": []
        });
        assert!(parse_immutable_broker_results(&wrong.to_string(), 5).is_err());

        let too_many = serde_json::json!({
            "schema": "astrid.edge.web_search.response.v1",
            "results": (0..6).map(|index| serde_json::json!({
                "title": format!("Result {index}"),
                "url": "https://example.com/",
                "snippet": "bounded"
            })).collect::<Vec<_>>()
        });
        assert!(parse_immutable_broker_results(&too_many.to_string(), 5).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_query, parse_brave_results};

    const FIXTURE: &str = r#"
<div class="snippet  result" data-type="web">
  <a href="https://example.org/paper?a=1&amp;b=2" class="card l1">
    <div class="title search-snippet-title">An <strong>Echo</strong> Paper</div>
  </a>
  <div class="content desktop-default-regular">A bounded &amp; useful <em>snippet</em>.</div>
</div>
<div class="snippet result" data-type="web">
  <a class="l1 other" href="https://example.net/second">
    <div class="title search-snippet-title">Second result</div>
  </a>
</div>
"#;

    #[test]
    fn query_encoding_is_utf8_and_url_safe() {
        assert_eq!(
            encode_query("echo state λ / 68%"),
            "echo+state+%CE%BB+%2F+68%25"
        );
    }

    #[test]
    fn brave_results_are_bounded_and_cleaned() {
        let results = parse_brave_results(FIXTURE, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["url"], "https://example.org/paper?a=1&b=2");
        assert_eq!(results[0]["title"], "An Echo Paper");
        assert_eq!(results[0]["snippet"], "A bounded & useful snippet.");
    }
}
