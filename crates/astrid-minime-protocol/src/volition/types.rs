use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    VOLITION_DECISION_SCHEMA_V1, VOLITION_INTENT_SCHEMA_V1, VOLITION_RECEIPT_SCHEMA_V1,
    canonical_sha256, finite_bounded_json, valid_bounded_text, valid_identifier, valid_sha256,
    valid_string_list,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionAuthorityClassV1 {
    SelfOwnedLocal,
    DelegatedExternal,
    Mutual,
    OperatorOneShot,
    SafetySupervisor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionDurabilityV1 {
    Standing,
    Lease,
    OneShot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionOperationV1 {
    Set,
    Execute,
    Withdraw,
    Hold,
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolitionActorIdentityV1 {
    pub being: String,
    pub principal: String,
    pub process_identity: String,
    pub deployment_identity: String,
}

impl VolitionActorIdentityV1 {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        valid_identifier(&self.being)
            && valid_identifier(&self.principal)
            && valid_identifier(&self.process_identity)
            && valid_identifier(&self.deployment_identity)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolitionBudgetV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microunits: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_count: Option<u32>,
}

impl VolitionBudgetV1 {
    #[must_use]
    pub fn is_nonzero_when_present(&self) -> bool {
        self.compute_millis.is_none_or(|value| value > 0)
            && self.network_bytes.is_none_or(|value| value > 0)
            && self.storage_bytes.is_none_or(|value| value > 0)
            && self.cost_microunits.is_none_or(|value| value > 0)
            && self.action_count.is_none_or(|value| value > 0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolitionActionV1 {
    pub operation: VolitionOperationV1,
    pub namespace: String,
    pub name: String,
    #[serde(default)]
    pub parameters: Value,
    pub reversible: bool,
    pub peer_impacting: bool,
    pub irreversible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_microunits: Option<u64>,
}

impl VolitionActionV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        valid_identifier(&self.namespace)
            && valid_identifier(&self.name)
            && finite_bounded_json(&self.parameters)
            && !(self.reversible && self.irreversible)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolitionIntentV1 {
    pub schema: String,
    pub intent_id: String,
    pub source_attestation_id: String,
    pub source_attestation_sha256: String,
    pub actor: VolitionActorIdentityV1,
    pub target_being: String,
    pub target_deployment_identity: String,
    pub action: VolitionActionV1,
    pub authority_class: VolitionAuthorityClassV1,
    pub durability: VolitionDurabilityV1,
    pub revision: u64,
    pub expected_revision: u64,
    pub issued_at_unix_ms: u64,
    pub command_expires_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_expires_at_unix_ms: Option<u64>,
    pub idempotency_key: String,
    #[serde(default)]
    pub capability_binding_ids: Vec<String>,
    #[serde(default)]
    pub authority_evidence_refs: Vec<String>,
    #[serde(default)]
    pub dependency_intent_ids: Vec<String>,
    pub budget: VolitionBudgetV1,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub success_conditions: Vec<String>,
    #[serde(default)]
    pub stop_conditions: Vec<String>,
}

impl VolitionIntentV1 {
    #[must_use]
    pub fn is_well_formed(&self, now_unix_ms: u64) -> bool {
        self.schema == VOLITION_INTENT_SCHEMA_V1
            && valid_identifier(&self.intent_id)
            && valid_identifier(&self.source_attestation_id)
            && valid_sha256(&self.source_attestation_sha256)
            && self.actor.is_complete()
            && valid_identifier(&self.target_being)
            && valid_identifier(&self.target_deployment_identity)
            && self.action.is_well_formed()
            && self.revision > 0
            && self.issued_at_unix_ms <= self.command_expires_at_unix_ms
            && now_unix_ms <= self.command_expires_at_unix_ms
            && valid_identifier(&self.idempotency_key)
            && self.budget.is_nonzero_when_present()
            && valid_string_list(&self.capability_binding_ids)
            && valid_string_list(&self.authority_evidence_refs)
            && valid_string_list(&self.dependency_intent_ids)
            && valid_string_list(&self.evidence_refs)
            && valid_string_list(&self.success_conditions)
            && valid_string_list(&self.stop_conditions)
            && self.durability_shape_is_valid(now_unix_ms)
            && self.authority_shape_is_valid()
    }

    fn durability_shape_is_valid(&self, now_unix_ms: u64) -> bool {
        match self.durability {
            VolitionDurabilityV1::Standing | VolitionDurabilityV1::OneShot => {
                self.action_expires_at_unix_ms.is_none()
            },
            VolitionDurabilityV1::Lease => self
                .action_expires_at_unix_ms
                .is_some_and(|expiry| expiry > now_unix_ms),
        }
    }

    fn authority_shape_is_valid(&self) -> bool {
        match self.authority_class {
            VolitionAuthorityClassV1::SelfOwnedLocal => {
                self.actor.being == self.target_being
                    && self.action.reversible
                    && !self.action.peer_impacting
                    && !self.action.irreversible
                    && !matches!(
                        self.action.operation,
                        VolitionOperationV1::Hold | VolitionOperationV1::Revert
                    )
            },
            VolitionAuthorityClassV1::DelegatedExternal => {
                self.actor.being == self.target_being
                    && self.action.reversible
                    && !self.action.peer_impacting
                    && !self.action.irreversible
                    && !self.capability_binding_ids.is_empty()
            },
            VolitionAuthorityClassV1::Mutual => {
                self.action.peer_impacting
                    && self.durability == VolitionDurabilityV1::OneShot
                    && self.authority_evidence_refs.len() >= 2
            },
            VolitionAuthorityClassV1::OperatorOneShot => {
                self.durability == VolitionDurabilityV1::OneShot
                    && !self.capability_binding_ids.is_empty()
            },
            VolitionAuthorityClassV1::SafetySupervisor => {
                self.actor.being == "safety_supervisor"
                    && self.durability == VolitionDurabilityV1::OneShot
                    && matches!(
                        self.action.operation,
                        VolitionOperationV1::Hold | VolitionOperationV1::Revert
                    )
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionDecisionStatusV1 {
    Accepted,
    ClarificationRequired,
    Held,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolitionDecisionV1 {
    pub schema: String,
    pub decision_id: String,
    pub intent_id: String,
    pub intent_sha256: String,
    pub status: VolitionDecisionStatusV1,
    pub authority_class: VolitionAuthorityClassV1,
    pub requested_action_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_action: Option<VolitionActionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_action_sha256: Option<String>,
    pub target_being: String,
    pub target_deployment_identity: String,
    pub no_target_substitution: bool,
    pub decided_by: String,
    pub decided_at_unix_ms: u64,
    #[serde(default)]
    pub authority_evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl VolitionDecisionV1 {
    #[must_use]
    pub fn is_well_formed_for(&self, intent: &VolitionIntentV1) -> bool {
        let effective_shape_valid = if self.status == VolitionDecisionStatusV1::Accepted {
            self.effective_action.as_ref().is_some_and(|action| {
                action.is_well_formed()
                    && self
                        .effective_action_sha256
                        .as_deref()
                        .is_some_and(|hash| hash == canonical_volition_action_sha256(action))
                    && self.requested_action_sha256
                        == self.effective_action_sha256.clone().unwrap_or_default()
            })
        } else {
            self.effective_action.is_none() && self.effective_action_sha256.is_none()
        };
        self.schema == VOLITION_DECISION_SCHEMA_V1
            && valid_identifier(&self.decision_id)
            && self.intent_id == intent.intent_id
            && self.intent_sha256 == canonical_volition_intent_sha256(intent)
            && self.authority_class == intent.authority_class
            && self.requested_action_sha256 == canonical_volition_action_sha256(&intent.action)
            && self.target_being == intent.target_being
            && self.target_deployment_identity == intent.target_deployment_identity
            && self.no_target_substitution
            && valid_identifier(&self.decided_by)
            && valid_string_list(&self.authority_evidence_refs)
            && self.reason.as_deref().is_none_or(valid_bounded_text)
            && effective_shape_valid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionReceiptStatusV1 {
    Applied,
    Duplicate,
    Held,
    Reverted,
    Rejected,
    Expired,
    RevisionConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerceptibleReturnV1 {
    pub summary: String,
    pub full_receipt_ref: String,
    pub delivered_at_unix_ms: u64,
    pub included_in_next_prompt: bool,
}

impl PerceptibleReturnV1 {
    fn is_well_formed(&self) -> bool {
        valid_bounded_text(&self.summary)
            && !self.summary.trim().is_empty()
            && valid_identifier(&self.full_receipt_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolitionReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub decision_id: String,
    pub intent_id: String,
    pub idempotency_key: String,
    pub status: VolitionReceiptStatusV1,
    pub requested_action_sha256: String,
    pub effective_action_sha256: String,
    pub requested_revision: u64,
    pub resulting_revision: u64,
    pub target_being: String,
    pub target_deployment_identity: String,
    #[serde(default)]
    pub machine_effects: Value,
    #[serde(default)]
    pub previous_state: Value,
    pub received_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub server_process_identity: String,
    pub server_deployment_identity: String,
    pub machine_effect_established: bool,
    pub felt_effect_established: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub perceptible_return: PerceptibleReturnV1,
}

impl VolitionReceiptV1 {
    #[must_use]
    pub fn is_well_formed_for(
        &self,
        intent: &VolitionIntentV1,
        decision: &VolitionDecisionV1,
    ) -> bool {
        let action_hashes_match = self.requested_action_sha256
            == canonical_volition_action_sha256(&intent.action)
            && self.effective_action_sha256 == self.requested_action_sha256;
        let applied_shape_valid = if self.status == VolitionReceiptStatusV1::Applied {
            decision.status == VolitionDecisionStatusV1::Accepted
                && self.machine_effect_established
                && action_hashes_match
        } else {
            true
        };
        self.schema == VOLITION_RECEIPT_SCHEMA_V1
            && valid_identifier(&self.receipt_id)
            && self.decision_id == decision.decision_id
            && self.intent_id == intent.intent_id
            && self.idempotency_key == intent.idempotency_key
            && self.target_being == intent.target_being
            && self.target_deployment_identity == intent.target_deployment_identity
            && self.completed_at_unix_ms >= self.received_at_unix_ms
            && valid_identifier(&self.server_process_identity)
            && valid_identifier(&self.server_deployment_identity)
            && finite_bounded_json(&self.machine_effects)
            && finite_bounded_json(&self.previous_state)
            && !self.felt_effect_established
            && self
                .rollback_receipt_id
                .as_deref()
                .is_none_or(valid_identifier)
            && self.reason.as_deref().is_none_or(valid_bounded_text)
            && self.perceptible_return.is_well_formed()
            && applied_shape_valid
    }
}

#[must_use]
pub fn canonical_volition_action_sha256(action: &VolitionActionV1) -> String {
    canonical_sha256(action)
}

#[must_use]
pub fn canonical_volition_intent_sha256(intent: &VolitionIntentV1) -> String {
    canonical_sha256(intent)
}
