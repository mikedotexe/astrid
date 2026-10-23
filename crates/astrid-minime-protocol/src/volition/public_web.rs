//! Standing public-web delegation, separate from V1 lifetime action budgets.
//!
//! Signatures establish provenance, not bootstrap trust. Consumers must also
//! validate the referenced capability, trusted issuer, current revocation state,
//! expected owner/deployment, and a durable state-revision floor. These contracts
//! V2 grants no authenticated API access. V3 explicitly adds the fixed Brave
//! search endpoint and a shared account ceiling, never model/control authority.

use serde::{Deserialize, Serialize};

use super::{canonical_json_bytes, canonical_sha256, valid_sha256, verify_signature};

mod operator;
pub use operator::{PUBLIC_WEB_APPROVAL_SCHEMA_V2, PublicWebApprovalV2, PublicWebMutationV2};

pub const PUBLIC_WEB_POLICY_SCHEMA_V2: &str = "volition.public_web_policy.v2";
pub const PUBLIC_WEB_POLICY_SCHEMA_V3: &str = "volition.public_web_policy.v3";
pub const PUBLIC_WEB_STATE_SCHEMA_V2: &str = "volition.public_web_policy_state.v2";
pub const PUBLIC_WEB_CAPABILITY_RESOURCE_V2: &str = "network://public-web/v2";
pub const PUBLIC_WEB_CAPABILITY_RESOURCE_V3: &str = "network://public-web/v3";
pub const BRAVE_REQUEST_MICROUSD: u64 = 5_000;

/// Operator-reported entitlement status, not a legal determination by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStorageRights {
    Unverified,
    OperatorConfirmed,
}

/// Explicit usage authorization; a key or signature does not establish storage rights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BraveSearchPolicyV1 {
    pub schema: String,
    pub account_id: String,
    pub monthly_limit_microusd: u64,
    pub request_cost_microusd: u64,
    pub storage_rights_status: SearchStorageRights,
    pub retention_review_audit_ref: String,
}

impl BraveSearchPolicyV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == "volition.brave_search.v1"
            && identifier(&self.account_id)
            && self.request_cost_microusd == BRAVE_REQUEST_MICROUSD
            && self.monthly_limit_microusd >= BRAVE_REQUEST_MICROUSD
            && identifier(&self.retention_review_audit_ref)
    }
}
pub const PUBLIC_WEB_MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
pub const PUBLIC_WEB_MAX_DURATION_MS: u64 = 120_000;
pub const PUBLIC_WEB_MIN_HEADROOM_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWebOperationV2 {
    Search,
    Page,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebOwnerV2 {
    pub being: String,
    pub principal: String,
    pub public_key_hex: String,
    pub deployment_identity: String,
}

impl PublicWebOwnerV2 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        identifier(&self.being)
            && identifier(&self.principal)
            && valid_sha256(&self.public_key_hex)
            && identifier(&self.deployment_identity)
    }

    /// Stable across deployments and policy rotation: one owner, one flight.
    #[must_use]
    pub fn lock_identity_sha256(&self) -> String {
        canonical_sha256(&(&self.being, &self.principal, &self.public_key_hex))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebLimitsV2 {
    pub decoded_body_bytes: u64,
    pub wire_body_bytes: u64,
    pub total_timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub search_hop_timeout_ms: u64,
    pub page_hop_timeout_ms: u64,
    pub redirects: u8,
    pub storage_headroom_bytes: u64,
}

impl Default for PublicWebLimitsV2 {
    fn default() -> Self {
        Self {
            decoded_body_bytes: PUBLIC_WEB_MAX_BODY_BYTES,
            wire_body_bytes: PUBLIC_WEB_MAX_BODY_BYTES,
            total_timeout_ms: PUBLIC_WEB_MAX_DURATION_MS,
            connect_timeout_ms: 5_000,
            search_hop_timeout_ms: 10_000,
            page_hop_timeout_ms: 15_000,
            redirects: 5,
            storage_headroom_bytes: PUBLIC_WEB_MIN_HEADROOM_BYTES,
        }
    }
}

impl PublicWebLimitsV2 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        (1..=PUBLIC_WEB_MAX_BODY_BYTES).contains(&self.decoded_body_bytes)
            && (1..=PUBLIC_WEB_MAX_BODY_BYTES).contains(&self.wire_body_bytes)
            && (1..=PUBLIC_WEB_MAX_DURATION_MS).contains(&self.total_timeout_ms)
            && (1..=5_000).contains(&self.connect_timeout_ms)
            && (1..=10_000).contains(&self.search_hop_timeout_ms)
            && (1..=15_000).contains(&self.page_hop_timeout_ms)
            && self.redirects <= 5
            && self.storage_headroom_bytes >= PUBLIC_WEB_MIN_HEADROOM_BYTES
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebPolicyV2 {
    pub schema: String,
    pub policy_id: String,
    pub revision: u64,
    pub owner: PublicWebOwnerV2,
    pub capability_token_id: String,
    pub capability_token_sha256: String,
    pub issuer_public_key_hex: String,
    pub operations: Vec<PublicWebOperationV2>,
    pub limits: PublicWebLimitsV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_api: Option<BraveSearchPolicyV1>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub approval_audit_refs: Vec<String>,
    pub signature_hex: String,
}

impl PublicWebPolicyV2 {
    #[must_use]
    pub fn capability_resource(&self) -> &'static str {
        if self.schema == PUBLIC_WEB_POLICY_SCHEMA_V3 {
            PUBLIC_WEB_CAPABILITY_RESOURCE_V3
        } else {
            PUBLIC_WEB_CAPABILITY_RESOURCE_V2
        }
    }

    #[must_use]
    pub fn signing_bytes(&self) -> Option<Vec<u8>> {
        let mut statement = self.clone();
        statement.signature_hex.clear();
        canonical_json_bytes(&statement)
    }

    #[must_use]
    pub fn sha256(&self) -> String {
        canonical_sha256(self)
    }

    #[must_use]
    pub fn verifies(&self, now_unix_ms: u64) -> bool {
        ((self.schema == PUBLIC_WEB_POLICY_SCHEMA_V2 && self.search_api.is_none())
            || (self.schema == PUBLIC_WEB_POLICY_SCHEMA_V3
                && self
                    .search_api
                    .as_ref()
                    .is_some_and(BraveSearchPolicyV1::is_well_formed)))
            && identifier(&self.policy_id)
            && self.revision > 0
            && self.owner.is_well_formed()
            && identifier(&self.capability_token_id)
            && valid_sha256(&self.capability_token_sha256)
            && !self.operations.is_empty()
            && self.operations.len() <= 2
            && (self.operations.len() == 1 || self.operations[0] != self.operations[1])
            && self.limits.is_well_formed()
            && self.issued_at_unix_ms > 0
            && self.issued_at_unix_ms <= now_unix_ms
            && now_unix_ms < self.expires_at_unix_ms
            && !self.approval_audit_refs.is_empty()
            && self.approval_audit_refs.len() <= 16
            && self
                .approval_audit_refs
                .iter()
                .all(|value| identifier(value))
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.issuer_public_key_hex, &self.signature_hex, &bytes)
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWebPolicyStatusV2 {
    Active,
    Revoked,
}

/// Current, durable policy selection. Missing/unreadable state must deny access.
/// A signed older active state is not a substitute for the current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebPolicyStateV2 {
    pub schema: String,
    pub policy_id: String,
    pub policy_sha256: String,
    pub revision: u64,
    pub status: PublicWebPolicyStatusV2,
    pub updated_at_unix_ms: u64,
    pub signer_public_key_hex: String,
    pub signature_hex: String,
}

impl PublicWebPolicyStateV2 {
    #[must_use]
    pub fn sha256(&self) -> String {
        canonical_sha256(self)
    }

    #[must_use]
    pub fn signing_bytes(&self) -> Option<Vec<u8>> {
        let mut statement = self.clone();
        statement.signature_hex.clear();
        canonical_json_bytes(&statement)
    }

    #[must_use]
    pub fn verifies_for(
        &self,
        policy: &PublicWebPolicyV2,
        minimum_revision: u64,
        now_unix_ms: u64,
    ) -> bool {
        let signer_allowed = self.signer_public_key_hex == policy.issuer_public_key_hex
            || (self.status == PublicWebPolicyStatusV2::Revoked
                && self.signer_public_key_hex == policy.owner.public_key_hex);
        self.schema == PUBLIC_WEB_STATE_SCHEMA_V2
            && self.policy_id == policy.policy_id
            && self.policy_sha256 == policy.sha256()
            && minimum_revision > 0
            && self.revision >= minimum_revision
            && self.revision >= policy.revision
            && self.updated_at_unix_ms >= policy.issued_at_unix_ms
            && self.updated_at_unix_ms <= now_unix_ms
            && signer_allowed
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.signer_public_key_hex, &self.signature_hex, &bytes)
            })
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
