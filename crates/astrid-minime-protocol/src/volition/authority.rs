use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{
    BEING_UTTERANCE_ATTESTATION_SCHEMA_V1, DELEGATED_CAPABILITY_BINDING_SCHEMA_V1,
    DELEGATED_CAPABILITY_USAGE_SCHEMA_V1, VolitionAuthorityClassV1, VolitionBudgetV1,
    canonical_json_bytes, canonical_sha256, valid_identifier, valid_sha256, valid_string_list,
    verify_signature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeingUtteranceAttestationV1 {
    pub schema: String,
    pub attestation_id: String,
    pub being: String,
    pub exchange_id: String,
    pub response_sha256: String,
    pub response_len_bytes: u64,
    pub model: String,
    pub provider: String,
    pub model_deployment_identity: String,
    pub captured_at_unix_ms: u64,
    pub attestor_process_identity: String,
    pub attestor_deployment_identity: String,
    pub attestor_public_key_hex: String,
    pub signature_hex: String,
}

impl BeingUtteranceAttestationV1 {
    #[must_use]
    pub fn signing_bytes(&self) -> Option<Vec<u8>> {
        canonical_json_bytes(&BeingUtteranceSigningStatementV1 {
            schema: &self.schema,
            attestation_id: &self.attestation_id,
            being: &self.being,
            exchange_id: &self.exchange_id,
            response_sha256: &self.response_sha256,
            response_len_bytes: self.response_len_bytes,
            model: &self.model,
            provider: &self.provider,
            model_deployment_identity: &self.model_deployment_identity,
            captured_at_unix_ms: self.captured_at_unix_ms,
            attestor_process_identity: &self.attestor_process_identity,
            attestor_deployment_identity: &self.attestor_deployment_identity,
            attestor_public_key_hex: &self.attestor_public_key_hex,
        })
    }

    #[must_use]
    pub fn verifies_response(
        &self,
        response: &[u8],
        now_unix_ms: u64,
        max_age_millis: u64,
    ) -> bool {
        let Ok(response_len) = u64::try_from(response.len()) else {
            return false;
        };
        let age_is_valid = now_unix_ms
            .checked_sub(self.captured_at_unix_ms)
            .is_some_and(|age| age <= max_age_millis);
        let response_sha256 = format!("{:x}", Sha256::digest(response));
        self.schema == BEING_UTTERANCE_ATTESTATION_SCHEMA_V1
            && valid_identifier(&self.attestation_id)
            && valid_identifier(&self.being)
            && valid_identifier(&self.exchange_id)
            && valid_sha256(&self.response_sha256)
            && self.response_sha256 == response_sha256
            && self.response_len_bytes == response_len
            && valid_identifier(&self.model)
            && valid_identifier(&self.provider)
            && valid_identifier(&self.model_deployment_identity)
            && valid_identifier(&self.attestor_process_identity)
            && valid_identifier(&self.attestor_deployment_identity)
            && age_is_valid
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.attestor_public_key_hex, &self.signature_hex, &bytes)
            })
    }
}

#[must_use]
pub fn canonical_being_utterance_attestation_sha256(
    attestation: &BeingUtteranceAttestationV1,
) -> String {
    canonical_sha256(attestation)
}

#[derive(Serialize)]
struct BeingUtteranceSigningStatementV1<'a> {
    schema: &'a str,
    attestation_id: &'a str,
    being: &'a str,
    exchange_id: &'a str,
    response_sha256: &'a str,
    response_len_bytes: u64,
    model: &'a str,
    provider: &'a str,
    model_deployment_identity: &'a str,
    captured_at_unix_ms: u64,
    attestor_process_identity: &'a str,
    attestor_deployment_identity: &'a str,
    attestor_public_key_hex: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedCapabilityBindingV1 {
    pub schema: String,
    pub binding_id: String,
    pub capability_token_id: String,
    pub capability_token_hash: String,
    pub being: String,
    pub being_principal: String,
    pub being_public_key_hex: String,
    pub deployment_identity: String,
    pub allowed_authority_classes: Vec<VolitionAuthorityClassV1>,
    pub resource_patterns: Vec<String>,
    pub budget: VolitionBudgetV1,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub single_use: bool,
    pub nonce: String,
    pub issuer_public_key_hex: String,
    pub signature_hex: String,
}

impl DelegatedCapabilityBindingV1 {
    #[must_use]
    pub fn signing_bytes(&self) -> Option<Vec<u8>> {
        canonical_json_bytes(&DelegatedCapabilitySigningStatementV1 {
            schema: &self.schema,
            binding_id: &self.binding_id,
            capability_token_id: &self.capability_token_id,
            capability_token_hash: &self.capability_token_hash,
            being: &self.being,
            being_principal: &self.being_principal,
            being_public_key_hex: &self.being_public_key_hex,
            deployment_identity: &self.deployment_identity,
            allowed_authority_classes: &self.allowed_authority_classes,
            resource_patterns: &self.resource_patterns,
            budget: &self.budget,
            issued_at_unix_ms: self.issued_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
            single_use: self.single_use,
            nonce: &self.nonce,
            issuer_public_key_hex: &self.issuer_public_key_hex,
        })
    }

    #[must_use]
    pub fn verifies(&self, now_unix_ms: u64) -> bool {
        let class_shape_valid = !self.allowed_authority_classes.is_empty()
            && self.allowed_authority_classes.iter().all(|class| {
                matches!(
                    class,
                    VolitionAuthorityClassV1::SelfOwnedLocal
                        | VolitionAuthorityClassV1::DelegatedExternal
                        | VolitionAuthorityClassV1::OperatorOneShot
                )
            });
        self.schema == DELEGATED_CAPABILITY_BINDING_SCHEMA_V1
            && valid_identifier(&self.binding_id)
            && valid_identifier(&self.capability_token_id)
            && valid_sha256(&self.capability_token_hash)
            && valid_identifier(&self.being)
            && valid_identifier(&self.being_principal)
            && valid_identifier(&self.deployment_identity)
            && valid_string_list(&self.resource_patterns)
            && !self.resource_patterns.is_empty()
            && self.budget.is_nonzero_when_present()
            && self.budget.action_count.is_some()
            && self.issued_at_unix_ms <= now_unix_ms
            && now_unix_ms <= self.expires_at_unix_ms
            && valid_identifier(&self.nonce)
            && class_shape_valid
            && self.signing_bytes().is_some_and(|bytes| {
                verify_signature(&self.issuer_public_key_hex, &self.signature_hex, &bytes)
            })
    }

    #[must_use]
    pub fn allows_authority_class(&self, class: VolitionAuthorityClassV1) -> bool {
        self.allowed_authority_classes.contains(&class)
    }

    #[must_use]
    pub fn allows_resource(&self, resource: &str) -> bool {
        !resource.contains("/../")
            && !resource.ends_with("/..")
            && self
                .resource_patterns
                .iter()
                .any(|pattern| wildcard_matches(pattern, resource))
    }
}

#[derive(Serialize)]
struct DelegatedCapabilitySigningStatementV1<'a> {
    schema: &'a str,
    binding_id: &'a str,
    capability_token_id: &'a str,
    capability_token_hash: &'a str,
    being: &'a str,
    being_principal: &'a str,
    being_public_key_hex: &'a str,
    deployment_identity: &'a str,
    allowed_authority_classes: &'a [VolitionAuthorityClassV1],
    resource_patterns: &'a [String],
    budget: &'a VolitionBudgetV1,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    single_use: bool,
    nonce: &'a str,
    issuer_public_key_hex: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedCapabilityUsageV1 {
    pub schema: String,
    pub binding_id: String,
    pub revision: u64,
    pub action_count: u32,
    pub compute_millis: u64,
    pub network_bytes: u64,
    pub storage_bytes: u64,
    pub cost_microunits: u64,
    pub single_use_consumed: bool,
    #[serde(default)]
    pub reserved_intent_ids: Vec<String>,
    #[serde(default)]
    pub completed_intent_ids: Vec<String>,
}

impl DelegatedCapabilityUsageV1 {
    #[must_use]
    pub fn new(binding: &DelegatedCapabilityBindingV1) -> Self {
        Self {
            schema: DELEGATED_CAPABILITY_USAGE_SCHEMA_V1.to_string(),
            binding_id: binding.binding_id.clone(),
            revision: 1,
            action_count: 0,
            compute_millis: 0,
            network_bytes: 0,
            storage_bytes: 0,
            cost_microunits: 0,
            single_use_consumed: false,
            reserved_intent_ids: Vec::new(),
            completed_intent_ids: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_well_formed_for(&self, binding: &DelegatedCapabilityBindingV1) -> bool {
        self.schema == DELEGATED_CAPABILITY_USAGE_SCHEMA_V1
            && self.binding_id == binding.binding_id
            && self.revision > 0
            && valid_string_list(&self.reserved_intent_ids)
            && valid_string_list(&self.completed_intent_ids)
            && identifiers_are_unique(&self.reserved_intent_ids)
            && identifiers_are_unique(&self.completed_intent_ids)
            && self
                .reserved_intent_ids
                .iter()
                .all(|id| !self.completed_intent_ids.contains(id))
            && within_limit(&self.action_count, binding.budget.action_count.as_ref())
            && within_limit(&self.compute_millis, binding.budget.compute_millis.as_ref())
            && within_limit(&self.network_bytes, binding.budget.network_bytes.as_ref())
            && within_limit(&self.storage_bytes, binding.budget.storage_bytes.as_ref())
            && within_limit(
                &self.cost_microunits,
                binding.budget.cost_microunits.as_ref(),
            )
            && (!self.single_use_consumed || binding.single_use)
    }

    #[must_use]
    pub fn can_reserve(
        &self,
        binding: &DelegatedCapabilityBindingV1,
        intent_id: &str,
        requested: &VolitionBudgetV1,
    ) -> bool {
        valid_identifier(intent_id)
            && self.is_well_formed_for(binding)
            && !self.reserved_intent_ids.iter().any(|id| id == intent_id)
            && !self.completed_intent_ids.iter().any(|id| id == intent_id)
            && (!binding.single_use
                || (!self.single_use_consumed
                    && self.reserved_intent_ids.is_empty()
                    && self.completed_intent_ids.is_empty()))
            && add_u32_within(self.action_count, 1, binding.budget.action_count)
            && optional_add_within(
                self.compute_millis,
                requested.compute_millis,
                binding.budget.compute_millis,
            )
            && optional_add_within(
                self.network_bytes,
                requested.network_bytes,
                binding.budget.network_bytes,
            )
            && optional_add_within(
                self.storage_bytes,
                requested.storage_bytes,
                binding.budget.storage_bytes,
            )
            && optional_add_within(
                self.cost_microunits,
                requested.cost_microunits,
                binding.budget.cost_microunits,
            )
            && requested.action_count.is_none_or(|count| count == 1)
    }

    pub fn reserve(
        &mut self,
        binding: &DelegatedCapabilityBindingV1,
        intent_id: String,
        requested: &VolitionBudgetV1,
    ) -> bool {
        if !self.can_reserve(binding, &intent_id, requested) {
            return false;
        }
        let Some(action_count) = self.action_count.checked_add(1) else {
            return false;
        };
        let Some(compute_millis) = add_optional(self.compute_millis, requested.compute_millis)
        else {
            return false;
        };
        let Some(network_bytes) = add_optional(self.network_bytes, requested.network_bytes) else {
            return false;
        };
        let Some(storage_bytes) = add_optional(self.storage_bytes, requested.storage_bytes) else {
            return false;
        };
        let Some(cost_microunits) = add_optional(self.cost_microunits, requested.cost_microunits)
        else {
            return false;
        };
        self.action_count = action_count;
        self.compute_millis = compute_millis;
        self.network_bytes = network_bytes;
        self.storage_bytes = storage_bytes;
        self.cost_microunits = cost_microunits;
        self.reserved_intent_ids.push(intent_id);
        self.revision = self.revision.saturating_add(1);
        true
    }

    pub fn complete(&mut self, binding: &DelegatedCapabilityBindingV1, intent_id: &str) -> bool {
        let Some(index) = self
            .reserved_intent_ids
            .iter()
            .position(|reserved| reserved == intent_id)
        else {
            return false;
        };
        let completed = self.reserved_intent_ids.remove(index);
        self.completed_intent_ids.push(completed);
        if binding.single_use {
            self.single_use_consumed = true;
        }
        self.revision = self.revision.saturating_add(1);
        true
    }
}

fn identifiers_are_unique(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn within_limit<T: PartialOrd>(used: &T, limit: Option<&T>) -> bool {
    limit.is_none_or(|limit| used <= limit)
}

fn add_u32_within(used: u32, requested: u32, limit: Option<u32>) -> bool {
    limit.is_some_and(|limit| {
        used.checked_add(requested)
            .is_some_and(|next| next <= limit)
    })
}

fn optional_add_within(used: u64, requested: Option<u64>, limit: Option<u64>) -> bool {
    requested.is_none_or(|requested| {
        limit.is_some_and(|limit| {
            used.checked_add(requested)
                .is_some_and(|next| next <= limit)
        })
    })
}

fn add_optional(used: u64, requested: Option<u64>) -> Option<u64> {
    requested.map_or(Some(used), |requested| used.checked_add(requested))
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut pattern_index = 0_usize;
    let mut value_index = 0_usize;
    let mut star_index = None;
    let mut star_value_index = 0_usize;
    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index = pattern_index.saturating_add(1);
            value_index = value_index.saturating_add(1);
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index = pattern_index.saturating_add(1);
            star_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star.saturating_add(1);
            star_value_index = star_value_index.saturating_add(1);
            value_index = star_value_index;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index = pattern_index.saturating_add(1);
    }
    pattern_index == pattern.len()
}
