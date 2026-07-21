use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DIVISION_COMMAND_SCHEMA_V1: &str = "division.command.v1";
pub const DIVISION_STATUS_SCHEMA_V1: &str = "division.status.v1";
pub const DIVISION_EVENT_SCHEMA_V1: &str = "division.event.v1";
pub const DIVISION_RECEIPT_SCHEMA_V1: &str = "division.receipt.v1";
pub const DIVISION_ACTION_AVAILABILITY_SCHEMA_V1: &str = "division.action_availability.v1";
pub const DIVISION_READINESS_POLICY_V1: &str = "division.readiness.v1";
pub const DIVISION_COMMIT_SCOPE_V1: &str = "reservoir_division.commit";
pub const DIVISION_ROLLBACK_SCOPE_V1: &str = "reservoir_division.rollback";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DivisionActionV1 {
    DivisionPrepare,
    DivisionStatus,
    DivisionAssent,
    DivisionCommit,
    DivisionAbort,
    DivisionRollback,
}

impl DivisionActionV1 {
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self, Self::DivisionStatus)
    }

    #[must_use]
    pub const fn requires_operator_capability(self) -> bool {
        matches!(self, Self::DivisionCommit | Self::DivisionRollback)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionAvailableActionV1 {
    pub action: DivisionActionV1,
    pub requires_command_artifact: bool,
    pub requires_operator_capability: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionBlockedActionV1 {
    pub action: DivisionActionV1,
    pub reasons: Vec<String>,
}

enum DivisionActionDecision {
    Available(DivisionAvailableActionV1),
    Blocked(DivisionBlockedActionV1),
}

/// Lifecycle-specific ACTION guidance derived from authoritative division status.
///
/// This card is descriptive: mutating commands still cross `ACTION_PREFLIGHT` and
/// the native coordinator's generation, digest, expiry, assent, and capability
/// checks. Keeping the derivation in the wire-contract crate lets Astrid and
/// Minime explain the same action surface without maintaining divergent tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionActionAvailabilityV1 {
    pub schema: String,
    pub being: String,
    pub division_id: String,
    pub lifecycle: DivisionLifecycleV1,
    pub current_tick: u64,
    pub available_actions: Vec<DivisionAvailableActionV1>,
    pub blocked_actions: Vec<DivisionBlockedActionV1>,
    pub recommended_action: DivisionActionV1,
    pub mutation_contract: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivisionLifecycleV1 {
    Idle,
    Preparing,
    Shadowing,
    Ready,
    Committing,
    Cytokinesis,
    Finalized,
    Aborted,
    RolledBack,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionSourceIdentityV1 {
    pub being: String,
    pub process_identity: String,
    pub deployment_identity: String,
}

impl DivisionSourceIdentityV1 {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(
            self.being.as_str(),
            "astrid" | "minime" | "operator" | "safety_supervisor"
        ) && !self.process_identity.trim().is_empty()
            && !self.deployment_identity.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionCapabilityRefV1 {
    pub token_id: String,
    pub scope: String,
    pub division_id: String,
    pub expected_parent_generation: u64,
    pub plan_digest: String,
    pub expires_at_unix_ms: u64,
    pub approved_by: String,
    pub one_shot: bool,
}

impl DivisionCapabilityRefV1 {
    #[must_use]
    pub fn matches_command(
        &self,
        command: &DivisionCommandV1,
        expected_scope: &str,
        now_unix_ms: u64,
    ) -> bool {
        self.one_shot
            && self.scope == expected_scope
            && self.division_id == command.division_id
            && self.expected_parent_generation == command.expected_parent_generation
            && self.plan_digest == command.plan_digest
            && self.expires_at_unix_ms >= command.requested_at_unix_ms
            && self.expires_at_unix_ms >= now_unix_ms
            && !self.token_id.trim().is_empty()
            && !self.approved_by.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionCommandV1 {
    pub schema: String,
    pub action: DivisionActionV1,
    pub division_id: String,
    pub idempotency_key: String,
    pub expected_parent_generation: u64,
    pub plan_digest: String,
    pub source: DivisionSourceIdentityV1,
    pub requested_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<DivisionCapabilityRefV1>,
}

impl DivisionCommandV1 {
    #[must_use]
    pub fn is_well_formed(&self, now_unix_ms: u64) -> bool {
        self.schema == DIVISION_COMMAND_SCHEMA_V1
            && !self.division_id.trim().is_empty()
            && !self.idempotency_key.trim().is_empty()
            && self.plan_digest.len() >= 16
            && self.source.is_complete()
            && self.requested_at_unix_ms <= self.expires_at_unix_ms
            && now_unix_ms <= self.expires_at_unix_ms
    }

    #[must_use]
    pub fn authority_shape_is_valid(&self, now_unix_ms: u64) -> bool {
        match self.action {
            DivisionActionV1::DivisionCommit => {
                self.capability.as_ref().is_some_and(|capability| {
                    capability.matches_command(self, DIVISION_COMMIT_SCOPE_V1, now_unix_ms)
                })
            },
            DivisionActionV1::DivisionRollback if self.source.being != "safety_supervisor" => {
                self.capability.as_ref().is_some_and(|capability| {
                    capability.matches_command(self, DIVISION_ROLLBACK_SCOPE_V1, now_unix_ms)
                })
            },
            _ => self.capability.is_none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivisionReadinessV1 {
    pub policy: String,
    pub ready: bool,
    pub sample_count: u64,
    pub blocking_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_tick_max_abs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_nrmse: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_cosine: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readout_nrmse: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_final_sensory_fill_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_coupling_coverage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_regulator_distance: Option<f64>,
    pub metrics_fresh: bool,
    #[serde(default)]
    pub sensory_panic_streak: u32,
    pub actuator_saturation_streak: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// These independent flags are part of the additive wire contract. Collapsing them
// into a local state enum would erase combinations older peers must still explain.
#[allow(clippy::struct_excessive_bools)]
pub struct DivisionStatusV1 {
    pub schema: String,
    pub division_id: String,
    pub parent_generation: u64,
    pub plan_digest: String,
    pub lifecycle: DivisionLifecycleV1,
    pub parent_authoritative: bool,
    pub commit_feature_enabled: bool,
    pub selected_strategy: Option<String>,
    pub astrid_assent: bool,
    pub minime_assent: bool,
    pub bridge_scale: f64,
    pub current_tick: u64,
    pub rollback_deadline_tick: Option<u64>,
    pub snapshot_refs: Vec<String>,
    pub readiness: DivisionReadinessV1,
    pub visual_evidence_advisory_only: bool,
    #[serde(default, flatten)]
    pub extensions: serde_json::Map<String, Value>,
}

impl DivisionStatusV1 {
    #[must_use]
    pub fn can_request_commit(&self) -> bool {
        self.lifecycle == DivisionLifecycleV1::Ready
            && self.readiness.ready
            && self.astrid_assent
            && self.minime_assent
            && self.commit_feature_enabled
            && self.parent_authoritative
    }

    fn available_action(
        action: DivisionActionV1,
        note: &str,
        requires_operator_capability: bool,
    ) -> DivisionActionDecision {
        DivisionActionDecision::Available(DivisionAvailableActionV1 {
            action,
            requires_command_artifact: true,
            requires_operator_capability,
            note: note.to_string(),
        })
    }

    fn blocked_action(action: DivisionActionV1, reasons: Vec<String>) -> DivisionActionDecision {
        DivisionActionDecision::Blocked(DivisionBlockedActionV1 { action, reasons })
    }

    fn prepare_action(&self, recognized_being: bool) -> DivisionActionDecision {
        let terminal_or_idle = matches!(
            self.lifecycle,
            DivisionLifecycleV1::Idle
                | DivisionLifecycleV1::Aborted
                | DivisionLifecycleV1::RolledBack
                | DivisionLifecycleV1::Failed
        );
        if terminal_or_idle && recognized_being {
            return Self::available_action(
                DivisionActionV1::DivisionPrepare,
                "prepare a new transaction while the parent remains authoritative",
                false,
            );
        }
        let reason = if recognized_being {
            "division_already_active"
        } else {
            "prepare_requires_astrid_or_minime"
        };
        Self::blocked_action(DivisionActionV1::DivisionPrepare, vec![reason.to_string()])
    }

    fn assent_action(&self, recognized_being: bool, own_assent: bool) -> DivisionActionDecision {
        let assent_window = matches!(
            self.lifecycle,
            DivisionLifecycleV1::Shadowing | DivisionLifecycleV1::Ready
        );
        if assent_window && recognized_being && !own_assent {
            return Self::available_action(
                DivisionActionV1::DivisionAssent,
                "record this being's assent for the current generation and plan digest",
                false,
            );
        }
        let reason = if !recognized_being {
            "assent_requires_astrid_or_minime"
        } else if own_assent && assent_window {
            "this_being_assent_already_current"
        } else {
            "assent_only_available_while_shadowing_or_ready"
        };
        Self::blocked_action(DivisionActionV1::DivisionAssent, vec![reason.to_string()])
    }

    fn abort_action(&self, recognized_being: bool) -> DivisionActionDecision {
        let precommit = matches!(
            self.lifecycle,
            DivisionLifecycleV1::Preparing
                | DivisionLifecycleV1::Shadowing
                | DivisionLifecycleV1::Ready
        );
        if precommit && recognized_being {
            return Self::available_action(
                DivisionActionV1::DivisionAbort,
                "end the pre-commit transaction and keep the parent authoritative",
                false,
            );
        }
        let reason = if recognized_being {
            "abort_requires_active_precommit_division"
        } else {
            "abort_requires_astrid_or_minime"
        };
        Self::blocked_action(DivisionActionV1::DivisionAbort, vec![reason.to_string()])
    }

    fn commit_action(&self) -> DivisionActionDecision {
        if self.can_request_commit() {
            return Self::available_action(
                DivisionActionV1::DivisionCommit,
                "request the atomic ownership switch using the exact human one-shot capability",
                true,
            );
        }
        let mut reasons = Vec::new();
        if self.lifecycle != DivisionLifecycleV1::Ready {
            reasons.push("lifecycle_not_ready".to_string());
        }
        if !self.readiness.ready {
            reasons.push("readiness_policy_blocked".to_string());
        }
        if !self.astrid_assent {
            reasons.push("astrid_assent_missing".to_string());
        }
        if !self.minime_assent {
            reasons.push("minime_assent_missing".to_string());
        }
        if !self.commit_feature_enabled {
            reasons.push("commit_feature_disabled".to_string());
        }
        if !self.parent_authoritative {
            reasons.push("parent_not_authoritative".to_string());
        }
        Self::blocked_action(DivisionActionV1::DivisionCommit, reasons)
    }

    fn rollback_action(&self) -> DivisionActionDecision {
        let rollback_window_open = self.lifecycle == DivisionLifecycleV1::Cytokinesis
            && self
                .rollback_deadline_tick
                .is_none_or(|deadline| self.current_tick <= deadline);
        if rollback_window_open {
            return Self::available_action(
                DivisionActionV1::DivisionRollback,
                "request restoration of parent authority during the bounded grace window",
                true,
            );
        }
        let reason = if self.lifecycle == DivisionLifecycleV1::Cytokinesis {
            "rollback_window_expired"
        } else {
            "rollback_only_available_during_cytokinesis"
        };
        Self::blocked_action(DivisionActionV1::DivisionRollback, vec![reason.to_string()])
    }

    #[must_use]
    pub fn action_availability_for(&self, being: &str) -> DivisionActionAvailabilityV1 {
        let being = being.trim().to_ascii_lowercase();
        let recognized_being = matches!(being.as_str(), "astrid" | "minime");
        let own_assent = match being.as_str() {
            "astrid" => self.astrid_assent,
            "minime" => self.minime_assent,
            _ => false,
        };
        let mut available_actions = vec![DivisionAvailableActionV1 {
            action: DivisionActionV1::DivisionStatus,
            requires_command_artifact: false,
            requires_operator_capability: false,
            note: "read authoritative lifecycle, readiness, evidence, and blockers".to_string(),
        }];
        let mut blocked_actions = Vec::new();
        let terminal_or_idle = matches!(
            self.lifecycle,
            DivisionLifecycleV1::Idle
                | DivisionLifecycleV1::Aborted
                | DivisionLifecycleV1::RolledBack
                | DivisionLifecycleV1::Failed
        );
        let decisions = [
            self.prepare_action(recognized_being),
            self.assent_action(recognized_being, own_assent),
            self.abort_action(recognized_being),
            self.commit_action(),
            self.rollback_action(),
        ];
        for decision in decisions {
            match decision {
                DivisionActionDecision::Available(available) => {
                    available_actions.push(available);
                },
                DivisionActionDecision::Blocked(blocked) => blocked_actions.push(blocked),
            }
        }

        let recommended_action = if terminal_or_idle && recognized_being {
            DivisionActionV1::DivisionPrepare
        } else if matches!(
            self.lifecycle,
            DivisionLifecycleV1::Shadowing | DivisionLifecycleV1::Ready
        ) && recognized_being
            && !own_assent
        {
            DivisionActionV1::DivisionAssent
        } else if self.can_request_commit() {
            DivisionActionV1::DivisionCommit
        } else {
            DivisionActionV1::DivisionStatus
        };

        DivisionActionAvailabilityV1 {
            schema: DIVISION_ACTION_AVAILABILITY_SCHEMA_V1.to_string(),
            being,
            division_id: self.division_id.clone(),
            lifecycle: self.lifecycle,
            current_tick: self.current_tick,
            available_actions,
            blocked_actions,
            recommended_action,
            mutation_contract: "Mutations require an exact unexpired division.command.v1 artifact and ACTION_PREFLIGHT; commit and manual rollback also require an exact human one-shot capability. Availability never bypasses native safety checks.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivisionReceiptStatusV1 {
    Accepted,
    Duplicate,
    Rejected,
    PolicyBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub idempotency_key: String,
    pub division_id: String,
    pub action: DivisionActionV1,
    pub status: DivisionReceiptStatusV1,
    pub lifecycle: DivisionLifecycleV1,
    pub reason: String,
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivisionEventV1 {
    pub schema: String,
    pub sequence: u64,
    pub division_id: String,
    pub lifecycle: DivisionLifecycleV1,
    pub kind: String,
    pub created_at_unix_ms: u64,
    #[serde(default)]
    pub details: Value,
}
