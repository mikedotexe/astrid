use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    OWNER_POLICY_RUNTIME_SCHEMA_V1, OWNER_POLICY_SCHEMA_V1, VolitionActionV1,
    VolitionAuthorityClassV1, valid_identifier, valid_string_list,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerPolicyComparatorV1 {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerPolicyLogicV1 {
    All,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerPolicyScopeV1 {
    SelfControl,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerPolicyConditionV1 {
    pub metric: String,
    pub comparator: OwnerPolicyComparatorV1,
    pub threshold: f64,
    pub hysteresis: f64,
    pub dwell_millis: u64,
}

impl OwnerPolicyConditionV1 {
    fn is_well_formed(&self) -> bool {
        valid_identifier(&self.metric)
            && self.threshold.is_finite()
            && self.hysteresis.is_finite()
            && self.hysteresis >= 0.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerPolicyV1 {
    pub schema: String,
    pub policy_id: String,
    pub owner_being: String,
    pub target_being: String,
    pub target_deployment_identity: String,
    pub source_attestation_id: String,
    pub scope: OwnerPolicyScopeV1,
    pub authority_class: VolitionAuthorityClassV1,
    pub logic: OwnerPolicyLogicV1,
    pub conditions: Vec<OwnerPolicyConditionV1>,
    pub action_template: VolitionActionV1,
    pub revision: u64,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub cooldown_millis: u64,
    pub max_executions: u32,
    pub enabled: bool,
    pub withdrawn: bool,
    #[serde(default)]
    pub stop_conditions: Vec<String>,
}

impl OwnerPolicyV1 {
    #[must_use]
    pub fn is_well_formed(&self, now_unix_ms: u64) -> bool {
        self.schema == OWNER_POLICY_SCHEMA_V1
            && valid_identifier(&self.policy_id)
            && valid_identifier(&self.owner_being)
            && self.owner_being == self.target_being
            && valid_identifier(&self.target_deployment_identity)
            && valid_identifier(&self.source_attestation_id)
            && self.scope == OwnerPolicyScopeV1::SelfControl
            && self.authority_class == VolitionAuthorityClassV1::SelfOwnedLocal
            && !self.conditions.is_empty()
            && self.conditions.len() <= 16
            && self
                .conditions
                .iter()
                .all(OwnerPolicyConditionV1::is_well_formed)
            && self.action_template.is_well_formed()
            && self.action_template.namespace == "self_control"
            && self.action_template.reversible
            && !self.action_template.peer_impacting
            && !self.action_template.irreversible
            && self.revision > 0
            && self.issued_at_unix_ms <= now_unix_ms
            && now_unix_ms <= self.expires_at_unix_ms
            && self.max_executions > 0
            && !(self.enabled && self.withdrawn)
            && valid_string_list(&self.stop_conditions)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerPolicyEvaluationStatusV1 {
    Ready,
    WaitingConditions,
    WaitingDwell,
    Cooldown,
    Disabled,
    Withdrawn,
    Expired,
    ExecutionLimitReached,
    Held,
    MissingMetric,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OwnerPolicyConditionRuntimeV1 {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entered_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_observed_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerPolicyRuntimeV1 {
    pub schema: String,
    pub policy_id: String,
    pub policy_revision: u64,
    pub condition_states: Vec<OwnerPolicyConditionRuntimeV1>,
    pub execution_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_execution_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_receipt_id: Option<String>,
    pub held: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_reason: Option<String>,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerPolicyEvaluationV1 {
    pub policy_id: String,
    pub policy_revision: u64,
    pub status: OwnerPolicyEvaluationStatusV1,
    pub should_execute: bool,
    pub condition_active: Vec<bool>,
    pub condition_dwell_satisfied: Vec<bool>,
    pub observed_metrics: BTreeMap<String, f64>,
    pub evaluated_at_unix_ms: u64,
}

impl OwnerPolicyRuntimeV1 {
    #[must_use]
    pub fn new(policy: &OwnerPolicyV1, now_unix_ms: u64) -> Self {
        Self {
            schema: OWNER_POLICY_RUNTIME_SCHEMA_V1.to_string(),
            policy_id: policy.policy_id.clone(),
            policy_revision: policy.revision,
            condition_states: vec![
                OwnerPolicyConditionRuntimeV1::default();
                policy.conditions.len()
            ],
            execution_count: 0,
            last_execution_at_unix_ms: None,
            last_receipt_id: None,
            held: false,
            hold_reason: None,
            updated_at_unix_ms: now_unix_ms,
        }
    }

    #[must_use]
    pub fn is_well_formed_for(&self, policy: &OwnerPolicyV1) -> bool {
        self.schema == OWNER_POLICY_RUNTIME_SCHEMA_V1
            && self.policy_id == policy.policy_id
            && self.policy_revision == policy.revision
            && self.condition_states.len() == policy.conditions.len()
            && self.execution_count <= policy.max_executions
            && self
                .condition_states
                .iter()
                .all(|state| state.last_observed_value.is_none_or(f64::is_finite))
            && self.last_receipt_id.as_deref().is_none_or(valid_identifier)
            && self.hold_reason.as_deref().is_none_or(valid_identifier)
            && !(self.held && self.hold_reason.is_none())
    }

    pub fn evaluate(
        &mut self,
        policy: &OwnerPolicyV1,
        metrics: &BTreeMap<String, f64>,
        now_unix_ms: u64,
    ) -> OwnerPolicyEvaluationV1 {
        if !self.is_well_formed_for(policy) {
            *self = Self::new(policy, now_unix_ms);
        }

        let mut active = Vec::with_capacity(policy.conditions.len());
        let mut dwell_satisfied = Vec::with_capacity(policy.conditions.len());
        let mut observed = BTreeMap::new();
        let mut missing_metric = false;

        for (condition, state) in policy
            .conditions
            .iter()
            .zip(self.condition_states.iter_mut())
        {
            let Some(value) = metrics
                .get(&condition.metric)
                .copied()
                .filter(|value| value.is_finite())
            else {
                state.active = false;
                state.entered_at_unix_ms = None;
                state.last_observed_value = None;
                active.push(false);
                dwell_satisfied.push(false);
                missing_metric = true;
                continue;
            };
            observed.insert(condition.metric.clone(), value);
            let is_active = condition_is_active(condition, value, state.active);
            if is_active {
                if !state.active {
                    state.entered_at_unix_ms = Some(now_unix_ms);
                }
            } else {
                state.entered_at_unix_ms = None;
            }
            state.active = is_active;
            state.last_observed_value = Some(value);
            active.push(is_active);
            dwell_satisfied.push(
                is_active
                    && state.entered_at_unix_ms.is_some_and(|entered_at| {
                        now_unix_ms.saturating_sub(entered_at) >= condition.dwell_millis
                    }),
            );
        }
        self.updated_at_unix_ms = now_unix_ms;

        let conditions_active = combine(policy.logic, &active);
        let dwell_ready = combine(policy.logic, &dwell_satisfied);
        let status = if policy.withdrawn {
            OwnerPolicyEvaluationStatusV1::Withdrawn
        } else if !policy.enabled {
            OwnerPolicyEvaluationStatusV1::Disabled
        } else if now_unix_ms > policy.expires_at_unix_ms {
            OwnerPolicyEvaluationStatusV1::Expired
        } else if self.held {
            OwnerPolicyEvaluationStatusV1::Held
        } else if self.execution_count >= policy.max_executions {
            OwnerPolicyEvaluationStatusV1::ExecutionLimitReached
        } else if !conditions_active {
            if missing_metric {
                OwnerPolicyEvaluationStatusV1::MissingMetric
            } else {
                OwnerPolicyEvaluationStatusV1::WaitingConditions
            }
        } else if !dwell_ready {
            OwnerPolicyEvaluationStatusV1::WaitingDwell
        } else if self
            .last_execution_at_unix_ms
            .is_some_and(|last| now_unix_ms.saturating_sub(last) < policy.cooldown_millis)
        {
            OwnerPolicyEvaluationStatusV1::Cooldown
        } else {
            OwnerPolicyEvaluationStatusV1::Ready
        };

        OwnerPolicyEvaluationV1 {
            policy_id: policy.policy_id.clone(),
            policy_revision: policy.revision,
            status,
            should_execute: status == OwnerPolicyEvaluationStatusV1::Ready,
            condition_active: active,
            condition_dwell_satisfied: dwell_satisfied,
            observed_metrics: observed,
            evaluated_at_unix_ms: now_unix_ms,
        }
    }

    pub fn record_execution(
        &mut self,
        policy: &OwnerPolicyV1,
        receipt_id: String,
        now_unix_ms: u64,
    ) -> bool {
        if self.policy_id != policy.policy_id
            || self.policy_revision != policy.revision
            || self.execution_count >= policy.max_executions
            || !valid_identifier(&receipt_id)
        {
            return false;
        }
        let Some(next_count) = self.execution_count.checked_add(1) else {
            return false;
        };
        self.execution_count = next_count;
        self.last_execution_at_unix_ms = Some(now_unix_ms);
        self.last_receipt_id = Some(receipt_id);
        self.updated_at_unix_ms = now_unix_ms;
        true
    }

    pub fn set_hold(&mut self, reason: Option<String>, now_unix_ms: u64) -> bool {
        match reason {
            Some(reason) if valid_identifier(&reason) => {
                self.held = true;
                self.hold_reason = Some(reason);
            },
            None => {
                self.held = false;
                self.hold_reason = None;
            },
            Some(_) => return false,
        }
        self.updated_at_unix_ms = now_unix_ms;
        true
    }
}

fn combine(logic: OwnerPolicyLogicV1, values: &[bool]) -> bool {
    match logic {
        OwnerPolicyLogicV1::All => values.iter().all(|value| *value),
        OwnerPolicyLogicV1::Any => values.iter().any(|value| *value),
    }
}

fn condition_is_active(condition: &OwnerPolicyConditionV1, value: f64, was_active: bool) -> bool {
    let threshold = condition.threshold;
    let hysteresis = condition.hysteresis;
    match condition.comparator {
        OwnerPolicyComparatorV1::LessThan => {
            value
                < if was_active {
                    threshold + hysteresis
                } else {
                    threshold
                }
        },
        OwnerPolicyComparatorV1::LessThanOrEqual => {
            value
                <= if was_active {
                    threshold + hysteresis
                } else {
                    threshold
                }
        },
        OwnerPolicyComparatorV1::GreaterThan => {
            value
                > if was_active {
                    threshold - hysteresis
                } else {
                    threshold
                }
        },
        OwnerPolicyComparatorV1::GreaterThanOrEqual => {
            value
                >= if was_active {
                    threshold - hysteresis
                } else {
                    threshold
                }
        },
        OwnerPolicyComparatorV1::Equal => (value - threshold).abs() <= hysteresis,
        OwnerPolicyComparatorV1::NotEqual => (value - threshold).abs() > hysteresis,
    }
}
