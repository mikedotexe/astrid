use crate::attestation::HmacSigner;
use crate::config::Config;
use crate::provider::{Provider, unload_request_sha256};
use crate::util::{
    append_private, atomic_private_write, canonical_json, read_stable_regular, sha256,
    unix_seconds, validate_hex64, validate_identifier,
};
use crate::{Error, Result};

pub const CONFIRMED: &str = "unload_confirmed";
pub const DEFERRED: &str = "unload_failed_build_deferred";

#[derive(Debug, Clone)]
pub struct HandoffOutcome {
    pub status: &'static str,
}

pub struct ModelIntentBinding<'a> {
    pub envelope_id: &'a str,
    pub intent_id: &'a str,
    pub trace_id: &'a str,
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub response_sha256: &'a str,
    pub terminal_declaration_sha256: &'a str,
    pub intent_envelope_sha256: &'a str,
    pub candidate_id: &'a str,
    pub candidate_sha256: &'a str,
}

/// Attempt one post-intent unload and persist the build-readiness evidence.
///
/// There is deliberately no retry. A transport or confirmation failure is a
/// valid signed `DEFERRED` outcome, not an error and never an authorship change.
#[allow(clippy::too_many_lines)] // Prepare-before-I/O and final receipt form one non-retryable transaction.
pub fn unload_and_record(
    config: &Config,
    signer: &HmacSigner,
    binding: &ModelIntentBinding<'_>,
) -> Result<HandoffOutcome> {
    for (value, label) in [
        (binding.envelope_id, "envelope_id"),
        (binding.intent_id, "intent_id"),
        (binding.trace_id, "trace_id"),
        (binding.session_id, "session_id"),
        (binding.turn_id, "turn_id"),
        (binding.candidate_id, "candidate_id"),
    ] {
        validate_identifier(value, label)?;
    }
    for (value, label) in [
        (binding.response_sha256, "response_sha256"),
        (
            binding.terminal_declaration_sha256,
            "terminal_declaration_sha256",
        ),
        (binding.intent_envelope_sha256, "intent_envelope_sha256"),
        (binding.candidate_sha256, "candidate_sha256"),
    ] {
        validate_hex64(value, label)?;
    }
    let request_sha256 = unload_request_sha256(config)?;
    let path = config
        .state_root
        .join("model-handoff")
        .join(format!("{}.json", binding.envelope_id));
    if path.exists() || path.is_symlink() {
        let bytes = read_stable_regular(&path, 128 * 1024)?;
        let status = verify_final(config, signer, binding, &request_sha256, &bytes)?;
        append_once(config, signer, binding.envelope_id, &bytes)?;
        return Ok(HandoffOutcome { status });
    }
    let prepared_path = config
        .state_root
        .join("model-handoff-transactions")
        .join(format!("{}.json", binding.envelope_id));
    let prepared_binding = handoff_binding(config, binding, &request_sha256);
    let prepared_binding_bytes = canonical_json(&prepared_binding)?;
    let prepared = serde_json::json!({
        "schema": "astrid.edge.steward_helper.model_unload_prepared_envelope.v1",
        "core": {
            "schema": "astrid.edge.steward_helper.model_unload_prepared.v1",
            "prepared_at": unix_seconds(),
            "binding": prepared_binding,
            "binding_sha256": sha256(&prepared_binding_bytes),
            "phase": "prepared_before_single_nonretryable_unload"
        }
    });
    let prepared_core_bytes = canonical_json(&prepared["core"])?;
    let prepared = serde_json::json!({
        "schema": prepared["schema"],
        "core": prepared["core"],
        "core_sha256": sha256(&prepared_core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&prepared_core_bytes)
        }
    });
    let proposed_prepared_bytes = canonical_json(&prepared)?;
    let prepared_preexisted = prepared_path.exists() || prepared_path.is_symlink();
    let prepared_bytes = if prepared_preexisted {
        read_stable_regular(&prepared_path, 128 * 1024)?
    } else {
        atomic_private_write(&prepared_path, &proposed_prepared_bytes)?;
        proposed_prepared_bytes
    };
    let prepared_at = verify_prepared(config, signer, binding, &request_sha256, &prepared_bytes)?;
    let (status, result_sha256, provider_result_sha256, elapsed_ms, result_class) =
        if prepared_preexisted {
            let class = "unload_outcome_unknown_after_restart_no_retry".to_owned();
            (DEFERRED, sha256(class.as_bytes()), None, None, class)
        } else {
            match Provider::new(config).unload() {
                Ok(response) => (
                    if response.request_sha256 == request_sha256 {
                        CONFIRMED
                    } else {
                        DEFERRED
                    },
                    response.result_sha256.clone(),
                    Some(response.result_sha256),
                    Some(response.elapsed_ms),
                    if response.request_sha256 == request_sha256 {
                        "exact_http_200_done_reason_unload".to_owned()
                    } else {
                        "internal_request_binding_mismatch".to_owned()
                    },
                ),
                Err(error) => {
                    let class = bounded_failure_class(&error);
                    (DEFERRED, sha256(class.as_bytes()), None, None, class)
                },
            }
        };
    let core = serde_json::json!({
        "schema": "astrid.edge.steward_helper.model_unload_handoff.v2",
        "recorded_at": prepared_at,
        "appliance_id": config.appliance_id,
        "envelope_id": binding.envelope_id,
        "intent_id": binding.intent_id,
        "trace_id": binding.trace_id,
        "session_id": binding.session_id,
        "turn_id": binding.turn_id,
        "response_sha256": binding.response_sha256,
        "terminal_declaration_sha256": binding.terminal_declaration_sha256,
        "intent_envelope_sha256": binding.intent_envelope_sha256,
        "candidate_id": binding.candidate_id,
        "candidate_sha256": binding.candidate_sha256,
        "model": config.model,
        "origin": config.ollama_origin,
        "request_sha256": request_sha256,
        "result_sha256": result_sha256,
        "provider_result_sha256": provider_result_sha256,
        "elapsed_ms": elapsed_ms,
        "result_class": result_class,
        "status": status,
        "attempt_count": 1,
        "automatic_retry": false,
        "build_ready": status == CONFIRMED,
        "authorship_unchanged": true,
        "response_body_retained": false
    });
    let core_bytes = canonical_json(&core)?;
    let receipt = serde_json::json!({
        "schema": "astrid.edge.steward_helper.model_unload_handoff_envelope.v2",
        "core": core,
        "core_sha256": sha256(&core_bytes),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(&core_bytes)
        }
    });
    let bytes = canonical_json(&receipt)?;
    atomic_private_write(&path, &bytes)?;
    append_once(config, signer, binding.envelope_id, &bytes)?;
    Ok(HandoffOutcome { status })
}

fn handoff_binding(
    config: &Config,
    binding: &ModelIntentBinding<'_>,
    request_sha256: &str,
) -> serde_json::Value {
    serde_json::json!({
        "appliance_id": config.appliance_id,
        "envelope_id": binding.envelope_id,
        "intent_id": binding.intent_id,
        "trace_id": binding.trace_id,
        "session_id": binding.session_id,
        "turn_id": binding.turn_id,
        "response_sha256": binding.response_sha256,
        "terminal_declaration_sha256": binding.terminal_declaration_sha256,
        "intent_envelope_sha256": binding.intent_envelope_sha256,
        "candidate_id": binding.candidate_id,
        "candidate_sha256": binding.candidate_sha256,
        "model": config.model,
        "origin": config.ollama_origin,
        "request_sha256": request_sha256
    })
}

fn verify_prepared(
    config: &Config,
    signer: &HmacSigner,
    binding: &ModelIntentBinding<'_>,
    request_sha256: &str,
    bytes: &[u8],
) -> Result<u64> {
    let envelope: serde_json::Value = serde_json::from_slice(bytes)?;
    let core = envelope
        .get("core")
        .ok_or_else(|| Error::new("model unload prepared record has no core"))?;
    let core_bytes = canonical_json(core)?;
    let expected_binding = handoff_binding(config, binding, request_sha256);
    let expected_binding_bytes = canonical_json(&expected_binding)?;
    let auth = envelope
        .get("auth")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::new("model unload prepared record has no authentication"))?;
    if envelope.get("schema").and_then(serde_json::Value::as_str)
        != Some("astrid.edge.steward_helper.model_unload_prepared_envelope.v1")
        || core.get("schema").and_then(serde_json::Value::as_str)
            != Some("astrid.edge.steward_helper.model_unload_prepared.v1")
        || core.get("phase").and_then(serde_json::Value::as_str)
            != Some("prepared_before_single_nonretryable_unload")
        || core.get("binding") != Some(&expected_binding)
        || core
            .get("binding_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(sha256(&expected_binding_bytes).as_str())
        || envelope
            .get("core_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(sha256(&core_bytes).as_str())
        || auth.get("algorithm").and_then(serde_json::Value::as_str) != Some("hmac-sha256")
        || auth.get("key_id").and_then(serde_json::Value::as_str) != Some(signer.key_id.as_str())
        || !auth
            .get("signature")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|signature| signer.verify(&core_bytes, signature))
    {
        return Err(Error::new(
            "model unload prepared record authentication failed",
        ));
    }
    core.get("prepared_at")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or_else(|| Error::new("model unload prepared timestamp is invalid"))
}

fn verify_final(
    config: &Config,
    signer: &HmacSigner,
    binding: &ModelIntentBinding<'_>,
    request_sha256: &str,
    bytes: &[u8],
) -> Result<&'static str> {
    let envelope: serde_json::Value = serde_json::from_slice(bytes)?;
    let core = envelope
        .get("core")
        .ok_or_else(|| Error::new("model unload receipt has no core"))?;
    let core_bytes = canonical_json(core)?;
    let expected = handoff_binding(config, binding, request_sha256);
    let auth = envelope
        .get("auth")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::new("model unload receipt has no authentication"))?;
    let status = core
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("model unload receipt status is absent"))?;
    if envelope.get("schema").and_then(serde_json::Value::as_str)
        != Some("astrid.edge.steward_helper.model_unload_handoff_envelope.v2")
        || core.get("schema").and_then(serde_json::Value::as_str)
            != Some("astrid.edge.steward_helper.model_unload_handoff.v2")
        || !matches!(status, CONFIRMED | DEFERRED)
        || expected.as_object().is_none_or(|fields| {
            fields
                .iter()
                .any(|(key, value)| core.get(key) != Some(value))
        })
        || envelope
            .get("core_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(sha256(&core_bytes).as_str())
        || auth.get("algorithm").and_then(serde_json::Value::as_str) != Some("hmac-sha256")
        || auth.get("key_id").and_then(serde_json::Value::as_str) != Some(signer.key_id.as_str())
        || !auth
            .get("signature")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|signature| signer.verify(&core_bytes, signature))
    {
        return Err(Error::new("model unload receipt authentication failed"));
    }
    Ok(if status == CONFIRMED {
        CONFIRMED
    } else {
        DEFERRED
    })
}

fn append_once(
    config: &Config,
    signer: &HmacSigner,
    envelope_id: &str,
    bytes: &[u8],
) -> Result<()> {
    let path = config.state_root.join("model-unload-receipts.jsonl");
    if path.exists() || path.is_symlink() {
        let ledger = read_stable_regular(&path, 64 * 1024 * 1024)?;
        if !ledger.is_empty() && !ledger.ends_with(b"\n") {
            return Err(Error::new(
                "model unload receipt ledger has an incomplete tail",
            ));
        }
        let mut exact = 0_u8;
        for line in ledger
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let record: serde_json::Value = serde_json::from_slice(line)?;
            let core = record
                .get("core")
                .ok_or_else(|| Error::new("model unload ledger record has no core"))?;
            if core.get("envelope_id").and_then(serde_json::Value::as_str) == Some(envelope_id) {
                let core_bytes = canonical_json(core)?;
                let auth = record
                    .get("auth")
                    .and_then(serde_json::Value::as_object)
                    .ok_or_else(|| {
                        Error::new("model unload ledger record has no authentication")
                    })?;
                if line != bytes
                    || record
                        .get("core_sha256")
                        .and_then(serde_json::Value::as_str)
                        != Some(sha256(&core_bytes).as_str())
                    || !auth
                        .get("signature")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|signature| signer.verify(&core_bytes, signature))
                {
                    return Err(Error::new("model unload ledger replay collision"));
                }
                exact = exact.saturating_add(1);
            }
        }
        if exact > 1 {
            return Err(Error::new("duplicate model unload receipts detected"));
        }
        if exact == 1 {
            return Ok(());
        }
    }
    let mut line = bytes.to_vec();
    line.push(b'\n');
    append_private(&path, &line)
}

fn bounded_failure_class(error: &Error) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timed out") || message.contains("deadline") {
        "transport_timeout".to_owned()
    } else if message.contains("connect") || message.contains("refused") {
        "transport_connect_failed".to_owned()
    } else if message.contains("confirm") || message.contains("done_reason") {
        "provider_unload_unconfirmed".to_owned()
    } else if message.contains("http") {
        "provider_http_failure".to_owned()
    } else {
        "provider_unload_failed".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_failure_class;
    use crate::Error;

    #[test]
    fn failure_receipts_use_bounded_classes_not_transport_bodies() {
        assert_eq!(
            bounded_failure_class(&Error::new("secret response body and deadline exceeded")),
            "transport_timeout"
        );
    }
}
