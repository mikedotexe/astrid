use astrid_guest::{capsule_result, http, ipc, serde_json, tool};
use serde_json::Value;

struct HttpCapsule;

impl astrid_guest::Guest for HttpCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_fetch_url" => handle_tool(&payload, fetch_url),
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
    let method = tool::string_arg(args, "method").unwrap_or_else(|| "GET".to_string());
    let body = tool::string_arg(args, "body");
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

    let response = http::request(&method, &url, headers, body)?;
    let body = String::from_utf8_lossy(&response.body).into_owned();
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
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = serde_json::json!({
        "capsule": "astrid-capsule-http",
        "tools": [
            {"name": "fetch_url", "description": "Fetch a URL through the Astrid HTTP host interface."}
        ]
    });
    match ipc::publish_json("tool.v1.response.describe.astrid-capsule-http", &payload) {
        Ok(()) => capsule_result::continue_empty(),
        Err(err) => capsule_result::deny(err),
    }
}

astrid_guest::export!(HttpCapsule);
