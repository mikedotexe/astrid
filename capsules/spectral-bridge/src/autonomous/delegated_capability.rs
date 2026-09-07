use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use astrid_capabilities::CapabilityToken;
use astrid_core::Permission;
use astrid_minime_protocol::{
    DelegatedCapabilityBindingV1, DelegatedCapabilityUsageV1, VolitionAuthorityClassV1,
    VolitionBudgetV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::volition;

const TRUST_SCHEMA_V1: &str = "astrid.volition.delegated_trust.v1";
const RESERVATION_SCHEMA_V1: &str = "astrid.volition.delegated_reservation.v1";
const TARGET_BEING: &str = "astrid";
const TARGET_PRINCIPAL: &str = "astrid";
const SEARCH_NETWORK_BUDGET: u64 = 4 * 1_024 * 1_024;
const SEARCH_COMPUTE_BUDGET_MS: u64 = 120_000;
const COMMUNICATION_STORAGE_BUDGET: u64 = 256 * 1_024;

static CAPABILITY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DelegatedTrustStoreV1 {
    schema: String,
    target_being: String,
    trusted_issuer_public_keys: Vec<String>,
    bootstrap_audit_refs: Vec<String>,
    authorized_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct DelegatedReservationV1 {
    schema: String,
    pub(super) binding_id: String,
    pub(super) capability_token_id: String,
    pub(super) resource: String,
    permission: Permission,
    pub(super) requested_budget: VolitionBudgetV1,
    intent_id: String,
    usage_revision: u64,
    reserved_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
struct DelegatedActionRequestV1 {
    resource: String,
    permission: Permission,
    budget: VolitionBudgetV1,
}

pub(super) fn reserve_for_action(
    volition_root: &Path,
    requested_action: &str,
    intent_id: &str,
    being_public_key_hex: &str,
    deployment_identity: &str,
    now: u64,
) -> Result<Option<DelegatedReservationV1>, String> {
    let request = action_request(requested_action).ok_or_else(|| {
        "delegated action did not resolve to one exact resource and permission".to_string()
    })?;
    reserve_at_root(
        volition_root,
        request,
        intent_id,
        being_public_key_hex,
        deployment_identity,
        now,
    )
}

pub(super) fn complete_reservation(
    volition_root: &Path,
    binding_id: &str,
    intent_id: &str,
) -> Result<(), String> {
    let _guard = capability_guard()?;
    if !safe_component(binding_id) || !safe_component(intent_id) {
        return Err("delegated capability completion identifier is unsafe".to_string());
    }
    let capability_root = capability_root(volition_root);
    let binding_path = capability_root
        .join("bindings")
        .join(format!("{binding_id}.json"));
    let binding = volition::read_json::<DelegatedCapabilityBindingV1>(&binding_path)?
        .ok_or_else(|| "delegated capability binding disappeared before completion".to_string())?;
    if !binding.verifies(binding.issued_at_unix_ms) {
        return Err("delegated capability binding failed integrity verification".to_string());
    }
    let usage_path = usage_path(&capability_root, binding_id);
    let mut usage = volition::read_json::<DelegatedCapabilityUsageV1>(&usage_path)?
        .ok_or_else(|| "delegated capability usage evidence is missing".to_string())?;
    if usage
        .completed_intent_ids
        .iter()
        .any(|completed| completed == intent_id)
    {
        return Ok(());
    }
    if !usage.complete(&binding, intent_id) || !usage.is_well_formed_for(&binding) {
        return Err("delegated capability reservation could not complete exactly".to_string());
    }
    volition::write_owner_json(&usage_path, &usage)
}

fn reserve_at_root(
    volition_root: &Path,
    request: DelegatedActionRequestV1,
    intent_id: &str,
    being_public_key_hex: &str,
    deployment_identity: &str,
    now: u64,
) -> Result<Option<DelegatedReservationV1>, String> {
    let _guard = capability_guard()?;
    if !safe_component(intent_id) {
        return Err("delegated capability intent identifier is unsafe".to_string());
    }
    let capability_root = capability_root(volition_root);
    let Some(trust) = load_trust(&capability_root)? else {
        return Ok(None);
    };
    let binding_root = capability_root.join("bindings");
    if !binding_root.exists() {
        return Ok(None);
    }
    let mut binding_paths = fs::read_dir(&binding_root)
        .map_err(|error| {
            format!(
                "read delegated capability bindings {}: {error}",
                binding_root.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    binding_paths.sort();

    for binding_path in binding_paths {
        let binding = volition::read_json::<DelegatedCapabilityBindingV1>(&binding_path)?
            .ok_or_else(|| "delegated capability binding vanished during selection".to_string())?;
        if !safe_component(&binding.binding_id)
            || !safe_component(&binding.capability_token_id)
            || binding_path.file_stem().and_then(|value| value.to_str())
                != Some(binding.binding_id.as_str())
        {
            return Err("delegated capability binding path or identifier is unsafe".to_string());
        }
        if !binding.verifies(binding.issued_at_unix_ms) {
            return Err(format!(
                "delegated capability binding `{}` failed signature or shape verification",
                binding.binding_id
            ));
        }
        if !binding.verifies(now) {
            continue;
        }
        if binding.being != TARGET_BEING
            || binding.being_principal != TARGET_PRINCIPAL
            || binding.being_public_key_hex != being_public_key_hex
            || binding.deployment_identity != deployment_identity
            || !binding.allows_authority_class(VolitionAuthorityClassV1::DelegatedExternal)
            || !binding.allows_resource(&request.resource)
        {
            continue;
        }
        let token_path = capability_root
            .join("tokens")
            .join(format!("{}.json", binding.capability_token_id));
        let token_bytes = fs::read(&token_path)
            .map_err(|error| format!("read capability token {}: {error}", token_path.display()))?;
        let token: CapabilityToken = serde_json::from_slice(&token_bytes).map_err(|error| {
            format!("decode capability token {}: {error}", token_path.display())
        })?;
        token
            .validate_with_skew(0)
            .map_err(|error| format!("validate capability token: {error}"))?;
        let token_issuer = token.issuer.to_hex();
        if binding.capability_token_id != token.id.to_string()
            || binding.capability_token_hash != sha256_bytes(&token_bytes)
            || binding.issuer_public_key_hex != token_issuer
            || !trust
                .trusted_issuer_public_keys
                .iter()
                .any(|trusted| trusted == &token_issuer)
            || (token.is_single_use() && !binding.single_use)
            || !token.grants(&request.resource, request.permission)
        {
            continue;
        }
        let usage_path = usage_path(&capability_root, &binding.binding_id);
        let mut usage = volition::read_json::<DelegatedCapabilityUsageV1>(&usage_path)?
            .unwrap_or_else(|| DelegatedCapabilityUsageV1::new(&binding));
        if !usage.is_well_formed_for(&binding) {
            return Err(format!(
                "delegated capability usage for `{}` failed integrity verification",
                binding.binding_id
            ));
        }
        if !usage.reserve(&binding, intent_id.to_string(), &request.budget) {
            continue;
        }
        volition::write_owner_json(&usage_path, &usage)?;
        return Ok(Some(DelegatedReservationV1 {
            schema: RESERVATION_SCHEMA_V1.to_string(),
            binding_id: binding.binding_id,
            capability_token_id: token.id.to_string(),
            resource: request.resource,
            permission: request.permission,
            requested_budget: request.budget,
            intent_id: intent_id.to_string(),
            usage_revision: usage.revision,
            reserved_at_unix_ms: now,
        }));
    }
    Ok(None)
}

fn load_trust(capability_root: &Path) -> Result<Option<DelegatedTrustStoreV1>, String> {
    let Some(trust) =
        volition::read_json::<DelegatedTrustStoreV1>(&capability_root.join("trust.json"))?
    else {
        return Ok(None);
    };
    let unique = trust
        .trusted_issuer_public_keys
        .iter()
        .collect::<BTreeSet<_>>();
    let keys_valid = !trust.trusted_issuer_public_keys.is_empty()
        && unique.len() == trust.trusted_issuer_public_keys.len()
        && trust
            .trusted_issuer_public_keys
            .iter()
            .all(|key| valid_public_key_hex(key));
    if trust.schema != TRUST_SCHEMA_V1
        || trust.target_being != TARGET_BEING
        || !keys_valid
        || trust.bootstrap_audit_refs.is_empty()
        || trust
            .bootstrap_audit_refs
            .iter()
            .any(|reference| reference.trim().is_empty())
        || trust.authorized_at_unix_ms == 0
    {
        return Err("delegated capability bootstrap trust is malformed".to_string());
    }
    Ok(Some(trust))
}

fn action_request(action: &str) -> Option<DelegatedActionRequestV1> {
    let base = action
        .split_whitespace()
        .next()
        .unwrap_or(action)
        .to_ascii_uppercase();
    let payload = action
        .get(base.len()..)
        .unwrap_or_default()
        .trim_start()
        .trim_start_matches([':', '-'])
        .trim();
    match base.as_str() {
        "SEARCH" | "RESEARCH" if !payload.is_empty() => {
            let topic_hash = sha256_bytes(payload.as_bytes());
            Some(DelegatedActionRequestV1 {
                resource: format!("network://search/{topic_hash}"),
                permission: Permission::Read,
                budget: VolitionBudgetV1 {
                    compute_millis: Some(SEARCH_COMPUTE_BUDGET_MS),
                    network_bytes: Some(SEARCH_NETWORK_BUDGET),
                    action_count: Some(1),
                    ..VolitionBudgetV1::default()
                },
            })
        },
        "BROWSE" if payload.starts_with("https://") || payload.starts_with("http://") => {
            Some(DelegatedActionRequestV1 {
                resource: payload.to_string(),
                permission: Permission::Read,
                budget: VolitionBudgetV1 {
                    compute_millis: Some(SEARCH_COMPUTE_BUDGET_MS),
                    network_bytes: Some(SEARCH_NETWORK_BUDGET),
                    action_count: Some(1),
                    ..VolitionBudgetV1::default()
                },
            })
        },
        "MESSAGE_MINIME"
        | "REPLY_MINIME"
        | "TRACE_MINIME"
        | "CORRESPONDENCE_TRACE"
        | "ACK_MINIME"
        | "CORRESPONDENCE_ACK"
        | "I_RECEIVED_THIS"
        | "CORRESPONDENCE_HEARTBEAT"
        | "SIGNAL_PERSISTENCE"
        | "PING"
        | "ASK" => Some(DelegatedActionRequestV1 {
            resource: "being://minime/inbox".to_string(),
            permission: Permission::Write,
            budget: VolitionBudgetV1 {
                storage_bytes: Some(COMMUNICATION_STORAGE_BUDGET),
                action_count: Some(1),
                ..VolitionBudgetV1::default()
            },
        }),
        _ => None,
    }
}

fn capability_root(volition_root: &Path) -> PathBuf {
    volition_root.join("capabilities")
}

fn usage_path(capability_root: &Path, binding_id: &str) -> PathBuf {
    capability_root
        .join("usage")
        .join(format!("{binding_id}.json"))
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_public_key_hex(value: &str) -> bool {
    value.len() == 64 && hex::decode(value).is_ok_and(|bytes| bytes.len() == 32)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn capability_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    CAPABILITY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "delegated capability lock poisoned".to_string())
}

#[cfg(test)]
pub(super) fn install_test_search_grant(
    volition_root: &Path,
    being_public_key_hex: &str,
    deployment_identity: &str,
    now: u64,
) -> String {
    use astrid_capabilities::{AuditEntryId, ResourcePattern, TokenScope};
    use astrid_crypto::KeyPair;

    let capability_root = capability_root(volition_root);
    for child in ["bindings", "tokens", "usage"] {
        volition::ensure_owner_dir(&capability_root.join(child)).unwrap();
    }
    let issuer = KeyPair::generate();
    let token = CapabilityToken::create(
        ResourcePattern::new("network://search/**").unwrap(),
        vec![Permission::Read],
        TokenScope::Persistent,
        issuer.key_id(),
        AuditEntryId::new(),
        &issuer,
        None,
    );
    let token_id = token.id.to_string();
    let token_path = capability_root
        .join("tokens")
        .join(format!("{token_id}.json"));
    volition::write_owner_json(&token_path, &token).unwrap();
    let token_bytes = fs::read(&token_path).unwrap();
    let issuer_public_key_hex = issuer.export_public_key().to_hex();
    let trust = DelegatedTrustStoreV1 {
        schema: TRUST_SCHEMA_V1.to_string(),
        target_being: TARGET_BEING.to_string(),
        trusted_issuer_public_keys: vec![issuer_public_key_hex.clone()],
        bootstrap_audit_refs: vec!["test:audit:bootstrap".to_string()],
        authorized_at_unix_ms: now,
    };
    volition::write_owner_json(&capability_root.join("trust.json"), &trust).unwrap();
    let binding_id = "test-search-binding".to_string();
    let mut binding = DelegatedCapabilityBindingV1 {
        schema: astrid_minime_protocol::DELEGATED_CAPABILITY_BINDING_SCHEMA_V1.to_string(),
        binding_id: binding_id.clone(),
        capability_token_id: token_id,
        capability_token_hash: sha256_bytes(&token_bytes),
        being: TARGET_BEING.to_string(),
        being_principal: TARGET_PRINCIPAL.to_string(),
        being_public_key_hex: being_public_key_hex.to_string(),
        deployment_identity: deployment_identity.to_string(),
        allowed_authority_classes: vec![VolitionAuthorityClassV1::DelegatedExternal],
        resource_patterns: vec!["network://search/**".to_string()],
        budget: VolitionBudgetV1 {
            compute_millis: Some(240_000),
            network_bytes: Some(8 * 1_024 * 1_024),
            action_count: Some(2),
            ..VolitionBudgetV1::default()
        },
        issued_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(60_000),
        single_use: false,
        nonce: "test-search-binding-nonce".to_string(),
        issuer_public_key_hex,
        signature_hex: String::new(),
    };
    binding.signature_hex = hex::encode(issuer.sign(&binding.signing_bytes().unwrap()).as_bytes());
    volition::write_owner_json(
        &capability_root
            .join("bindings")
            .join(format!("{binding_id}.json")),
        &binding,
    )
    .unwrap();
    binding_id
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn delegated_grant_binds_existing_token_and_reserves_before_dispatch() {
        let root = tempdir().unwrap();
        let being_key = SigningKey::from_bytes(&[37_u8; 32]);
        let being_public_key_hex = hex::encode(being_key.verifying_key().to_bytes());
        let binding_id =
            install_test_search_grant(root.path(), &being_public_key_hex, "deployment-a", 10_000);
        let reservation = reserve_for_action(
            root.path(),
            "SEARCH reservoir continuity",
            "intent-search-1",
            &being_public_key_hex,
            "deployment-a",
            10_001,
        )
        .unwrap()
        .expect("audited grant should reserve");
        assert_eq!(reservation.binding_id, binding_id);
        assert_eq!(reservation.requested_budget.action_count, Some(1));
        assert!(
            reserve_for_action(
                root.path(),
                "SEARCH reservoir continuity",
                "intent-search-1",
                &being_public_key_hex,
                "deployment-a",
                10_002,
            )
            .unwrap()
            .is_none()
        );
        complete_reservation(root.path(), &binding_id, "intent-search-1").unwrap();
        complete_reservation(root.path(), &binding_id, "intent-search-1").unwrap();
        let usage = volition::read_json::<DelegatedCapabilityUsageV1>(&usage_path(
            &capability_root(root.path()),
            &binding_id,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(usage.completed_intent_ids, vec!["intent-search-1"]);
        assert!(usage.reserved_intent_ids.is_empty());
    }

    #[test]
    fn delegated_grant_rejects_stale_deployment_and_unbound_resource() {
        let root = tempdir().unwrap();
        let being_key = SigningKey::from_bytes(&[38_u8; 32]);
        let being_public_key_hex = hex::encode(being_key.verifying_key().to_bytes());
        install_test_search_grant(root.path(), &being_public_key_hex, "deployment-a", 20_000);
        assert!(
            reserve_for_action(
                root.path(),
                "SEARCH exact grant",
                "intent-stale",
                &being_public_key_hex,
                "deployment-b",
                20_001,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            reserve_for_action(
                root.path(),
                "BROWSE https://example.com/",
                "intent-browser",
                &being_public_key_hex,
                "deployment-a",
                20_001,
            )
            .unwrap()
            .is_none()
        );
    }
}
