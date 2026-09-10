// Three evidence levels: caller preference, serialized adapter request, and an
// optional server receipt. Omitted Ollama parameters remain unknown defaults.
fn provider_control_request(bytes: &[u8]) -> serde_json::Value {
    let request = serde_json::from_slice::<serde_json::Value>(bytes).unwrap_or_default();
    let mut sent = serde_json::Map::new();
    for key in [
        "temperature",
        "top_p",
        "top_k",
        "min_p",
        "repetition_penalty",
        "repetition_context_size",
        "max_tokens",
        "think",
        "stream",
        "aperture",
    ] {
        if let Some(value) = request.get(key) {
            sent.insert(key.to_string(), value.clone());
        }
    }
    if let Some(options) = request
        .get("options")
        .and_then(serde_json::Value::as_object)
    {
        for (key, value) in options {
            if value.is_number() || value.is_boolean() || value.is_null() {
                sent.insert(key.clone(), value.clone());
            }
        }
    }
    serde_json::json!({"schema": "provider_generation_controls_v1", "requested": null,
        "adapter_sent": sent, "adapter_evidence": "serialized_request_not_server_confirmation",
        "server_reported": null, "omitted_settings": "provider_defaults_unconfirmed"})
}

fn provider_control_response(body: &str) -> serde_json::Value {
    let Ok(response) = serde_json::from_str::<serde_json::Value>(body) else {
        return serde_json::Value::Null;
    };
    response
        .get("coupled_generation_v1")
        .filter(|value| value.is_object() && value.to_string().len() <= 32_768)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod generation_control_tests {
    use super::*;

    #[test]
    fn serialized_settings_are_not_mistaken_for_server_confirmation() {
        let request = provider_control_request(br#"{"think":false,"options":{"temperature":0.9,"top_p":0.95,"num_predict":8192},"messages":[{"content":"private"}]}"#);
        assert_eq!(request["adapter_sent"]["top_p"], 0.95);
        assert_eq!(request["adapter_sent"]["num_predict"], 8192);
        assert!(request["server_reported"].is_null());
        assert!(!request.to_string().contains("private"));
        assert!(provider_control_response(r#"{"done":true,"done_reason":"stop"}"#).is_null());
        let receipt = provider_control_response(
            r#"{"coupled_generation_v1":{"controls":{"top_p":0.95},"source":"server_reported"}}"#,
        );
        assert_eq!(receipt["controls"]["top_p"], 0.95);
    }
}
