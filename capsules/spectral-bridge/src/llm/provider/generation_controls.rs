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

fn provider_completion_metadata(body: &str) -> serde_json::Value {
    let response = serde_json::from_str::<serde_json::Value>(body).unwrap_or_default();
    let reason = response.get("done_reason").or_else(|| response.pointer("/choices/0/finish_reason"));
    let reason = reason.and_then(serde_json::Value::as_str).map(|reason| match reason {
        "stop" | "length" | "eos" | "eos_token" | "max_tokens" | "tool_calls" | "content_filter" | "load" | "unload" => reason,
        _ => "other_reported",
    });
    serde_json::json!({
        "native_finish": reason,
        "native_done": response.get("done").and_then(serde_json::Value::as_bool),
        "provider_eval_count": response.get("eval_count").or_else(|| response.pointer("/usage/completion_tokens")).and_then(serde_json::Value::as_u64),
        "scope": "provider_report_not_authored_intent_or_output_acceptance",
    })
}

fn record_provider_completion(label: &str, provider: &str, max_tokens: u32, body: &str) {
    if cfg!(test) {
        return;
    }
    append_llm_diagnostic_jsonl("provider_completion.jsonl", &serde_json::json!({
        "schema": "provider_completion_v1", "timestamp": unix_timestamp_string(),
        "label": label, "provider": provider, "effective_output_ceiling": max_tokens,
        "completion": provider_completion_metadata(body),
    }));
}

#[cfg(test)]
mod generation_control_tests {
    use super::*;

    #[test]
    fn completion_receipt_is_bounded_and_missing_reason_stays_unknown() {
        assert!(provider_completion_metadata("{}")["native_finish"].is_null());
        let receipt = provider_completion_metadata(r#"{"choices":[{"finish_reason":"length","message":{"content":"private prose"}}],"usage":{"completion_tokens":8192}}"#);
        assert_eq!(receipt["native_finish"], "length");
        assert_eq!(receipt["provider_eval_count"], 8192);
        assert!(!receipt.to_string().contains("private prose"));
        assert_eq!(provider_completion_metadata(r#"{"done_reason":"stop","done":true}"#)["native_finish"], "stop");
        assert_eq!(provider_completion_metadata(r#"{"done_reason":"private text"}"#)["native_finish"], "other_reported");
        assert!(provider_completion_metadata(r#"{"eval_count":-1}"#)["provider_eval_count"].is_null());
    }

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
