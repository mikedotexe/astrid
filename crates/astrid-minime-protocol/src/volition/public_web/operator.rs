use serde::{Deserialize, Serialize};

use super::{
    PublicWebOwnerV2, PublicWebPolicyStateV2, PublicWebPolicyStatusV2, PublicWebPolicyV2,
    canonical_json_bytes, canonical_sha256, identifier, valid_sha256, verify_signature,
};

pub const PUBLIC_WEB_APPROVAL_SCHEMA_V2: &str = "volition.public_web_operator_approval.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWebMutationV2 {
    Install,
    Revoke,
}

/// A fresh operator approval of exact bytes, paths/configuration and prior head.
/// It is not an approval inferred from policy existence or a command-line label.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebApprovalV2 {
    pub schema: String,
    pub operation: PublicWebMutationV2,
    pub owner: PublicWebOwnerV2,
    pub policy_sha256: String,
    pub state_sha256: String,
    pub config_sha256: String,
    pub expected_previous_floor_sha256: Option<String>,
    pub approved_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub approval_audit_ref: String,
    pub signer_public_key_hex: String,
    pub signature_hex: String,
}

impl PublicWebApprovalV2 {
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
    pub fn verifies_for(
        &self,
        policy: &PublicWebPolicyV2,
        state: &PublicWebPolicyStateV2,
        config_sha256: &str,
        now_unix_ms: u64,
    ) -> bool {
        let right_status = matches!(
            (self.operation, state.status),
            (
                PublicWebMutationV2::Install,
                PublicWebPolicyStatusV2::Active
            ) | (
                PublicWebMutationV2::Revoke,
                PublicWebPolicyStatusV2::Revoked
            )
        );
        let signer_allowed = self.signer_public_key_hex == policy.issuer_public_key_hex
            || (self.operation == PublicWebMutationV2::Revoke
                && self.signer_public_key_hex == policy.owner.public_key_hex);
        self.schema == PUBLIC_WEB_APPROVAL_SCHEMA_V2
            && right_status
            && signer_allowed
            && self.owner == policy.owner
            && self.policy_sha256 == policy.sha256()
            && self.state_sha256 == state.sha256()
            && self.config_sha256 == config_sha256
            && self
                .expected_previous_floor_sha256
                .as_ref()
                .is_none_or(|hash| valid_sha256(hash))
            && self.approved_at_unix_ms > 0
            && self.approved_at_unix_ms <= now_unix_ms
            && now_unix_ms < self.expires_at_unix_ms
            && self
                .expires_at_unix_ms
                .checked_sub(self.approved_at_unix_ms)
                .is_some_and(|duration| duration <= 300_000)
            && identifier(&self.approval_audit_ref)
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.signer_public_key_hex, &self.signature_hex, &bytes)
            })
    }
}
