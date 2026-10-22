use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use astrid_capabilities::{CapabilityToken, TokenScope};
use astrid_core::Permission;
use astrid_minime_protocol::public_web::{
    PUBLIC_WEB_POLICY_SCHEMA_V2, PUBLIC_WEB_POLICY_SCHEMA_V3, PublicWebOperationV2,
    PublicWebPolicyStateV2, PublicWebPolicyStatusV2, PublicWebPolicyV2,
};
use serde::{Deserialize, Serialize};

use crate::{
    CONFIG_SCHEMA, CONFIG_SCHEMA_V3, PublicWebConfig, PublicWebErrorCode as E, Result, digest,
    guard,
};

/// Wire-compatible with the existing delegated-capability bootstrap trust file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trust {
    schema: String,
    target_being: String,
    trusted_issuer_public_keys: Vec<String>,
    bootstrap_audit_refs: Vec<String>,
    authorized_at_unix_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct PublicWebPreview {
    pub policy_sha256: String,
    pub owner_sha256: String,
    pub state_revision: u64,
    pub expires_at_unix_ms: u64,
    pub operations: Vec<PublicWebOperationV2>,
    pub limits: astrid_minime_protocol::public_web::PublicWebLimitsV2,
    pub implementation_scope: &'static str,
}

pub(crate) trait Authority {
    fn current(&self, operation: PublicWebOperationV2) -> Result<PublicWebPolicyV2>;
}

impl Authority for PublicWebConfig {
    fn current(&self, operation: PublicWebOperationV2) -> Result<PublicWebPolicyV2> {
        let (policy, _) = load_authority(self)?;
        if !policy.operations.contains(&operation) {
            return Err(E::OperationNotGranted);
        }
        Ok(policy)
    }
}

/// Read-only authority preview. Does not acquire a lease, install, or grant.
///
/// # Errors
/// Returns a content-free code for invalid, missing, revoked, or untrusted data.
pub fn preview(config: &PublicWebConfig) -> Result<PublicWebPreview> {
    let (policy, state) = load_authority(config)?;
    Ok(PublicWebPreview {
        policy_sha256: policy.sha256(),
        owner_sha256: policy.owner.lock_identity_sha256(),
        state_revision: state.revision,
        expires_at_unix_ms: policy.expires_at_unix_ms,
        operations: policy.operations,
        limits: policy.limits,
        implementation_scope: "transport_foundation",
    })
}

fn load_authority(config: &PublicWebConfig) -> Result<(PublicWebPolicyV2, PublicWebPolicyStateV2)> {
    load_selected(config, false)
}

pub(crate) fn load_selected(
    config: &PublicWebConfig,
    allow_revoked: bool,
) -> Result<(PublicWebPolicyV2, PublicWebPolicyStateV2)> {
    let floor = crate::operator::read_floor(config)?.ok_or(E::AuthorityUnavailable)?;
    if floor.pending {
        return Err(E::StatePending);
    }
    let policy: PublicWebPolicyV2 =
        serde_json::from_slice(&guard::read_private(&config.policy_path)?)
            .map_err(|_| E::InvalidPolicy)?;
    let state: PublicWebPolicyStateV2 =
        serde_json::from_slice(&guard::read_private(&config.state_path)?)
            .map_err(|_| E::InvalidState)?;
    if !canonical_token_id(&policy.capability_token_id) {
        return Err(E::InvalidCapability);
    }
    let token_bytes = guard::read_private(&token_path(config, &policy))?;
    validate_candidate(config, &policy, &state, &token_bytes, allow_revoked)?;
    if floor.revision != state.revision
        || floor.state_sha256 != state.sha256()
        || floor.policy_sha256 != policy.sha256()
    {
        return Err(E::InvalidState);
    }
    let current = crate::operator::read_floor(config)?.ok_or(E::AuthorityUnavailable)?;
    if current.pending {
        return Err(E::StatePending);
    }
    if current.sha256() != floor.sha256() {
        return Err(E::AuthorityChanged);
    }
    Ok((policy, state))
}

pub(crate) fn token_path(
    config: &PublicWebConfig,
    policy: &PublicWebPolicyV2,
) -> std::path::PathBuf {
    config
        .delegation_root
        .join("tokens")
        .join(format!("{}.json", policy.capability_token_id))
}

pub(crate) fn validate_candidate(
    config: &PublicWebConfig,
    policy: &PublicWebPolicyV2,
    state: &PublicWebPolicyStateV2,
    token_bytes: &[u8],
    allow_revoked: bool,
) -> Result<()> {
    if !((config.schema == CONFIG_SCHEMA
        && config.brave.is_none()
        && policy.schema == PUBLIC_WEB_POLICY_SCHEMA_V2)
        || (config.schema == CONFIG_SCHEMA_V3
            && config.brave.is_some()
            && policy.schema == PUBLIC_WEB_POLICY_SCHEMA_V3))
        || !config.expected_owner.is_well_formed()
        || config.minimum_state_revision == 0
    {
        return Err(E::InvalidConfig);
    }
    let now = now_ms()?;
    let trust: Trust = serde_json::from_slice(&guard::read_private(
        &config.delegation_root.join("trust.json"),
    )?)
    .map_err(|_| E::UntrustedIssuer)?;
    let unique = trust
        .trusted_issuer_public_keys
        .iter()
        .collect::<BTreeSet<_>>();
    if trust.schema != "astrid.volition.delegated_trust.v1"
        || trust.target_being != config.expected_owner.being
        || trust.authorized_at_unix_ms == 0
        || trust.authorized_at_unix_ms > now
        || unique.is_empty()
        || unique.len() != trust.trusted_issuer_public_keys.len()
        || unique
            .iter()
            .any(|key| key.len() != 64 || !key.bytes().all(|value| value.is_ascii_hexdigit()))
        || trust.bootstrap_audit_refs.is_empty()
        || trust
            .bootstrap_audit_refs
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(E::UntrustedIssuer);
    }
    if !policy.verifies(now) || policy.sha256() != config.expected_policy_sha256 {
        return Err(E::InvalidPolicy);
    }
    if policy.owner != config.expected_owner {
        return Err(E::OwnerMismatch);
    }
    if !trust
        .trusted_issuer_public_keys
        .contains(&policy.issuer_public_key_hex)
    {
        return Err(E::UntrustedIssuer);
    }
    if !state.verifies_for(policy, config.minimum_state_revision, now) {
        return Err(E::InvalidState);
    }
    if state.status == PublicWebPolicyStatusV2::Revoked && !allow_revoked {
        return Err(E::Revoked);
    }
    if !canonical_token_id(&policy.capability_token_id) {
        return Err(E::InvalidCapability);
    }
    let token: CapabilityToken =
        serde_json::from_slice(token_bytes).map_err(|_| E::InvalidCapability)?;
    if digest(token_bytes) != policy.capability_token_sha256
        || token.id.to_string() != policy.capability_token_id
        || token.issuer.to_hex() != policy.issuer_public_key_hex
        || token.validate_with_skew(0).is_err()
        || token.scope != TokenScope::Persistent
        || token.is_single_use()
        || token.resource.as_str() != policy.capability_resource()
        || token.permissions != vec![Permission::Read]
        || !token.grants(policy.capability_resource(), Permission::Read)
        || token.issued_at.0.timestamp_millis() > i64::try_from(now).unwrap_or(i64::MAX)
    {
        return Err(E::InvalidCapability);
    }
    Ok(())
}

fn canonical_token_id(value: &str) -> bool {
    value.strip_prefix("token:").is_some_and(|uuid| {
        uuid.len() == 36
            && uuid.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
                }
            })
    })
}

pub(crate) fn now_ms() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| u64::try_from(value.as_millis()).ok())
        .ok_or(E::AuthorityUnavailable)
}
