//! Trusted, evidence-only schemas for reciprocal experiential systems.
//!
//! These records are deliberately not deserializable. Persisted JSON is
//! untrusted input and must be revalidated by the projection layer before a
//! crate-local builder may construct one of these forms.

use serde::Serialize;

mod context;

pub use context::{ReciprocalContextKindV1, ReciprocalContextReceiptV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct ExperientialEvidenceAuthorityV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
    live_eligible_now: bool,
    auto_approved: bool,
    grants_approval: bool,
    edits_source_now: bool,
}

impl ExperientialEvidenceAuthorityV1 {
    const fn evidence_only() -> Self {
        Self {
            schema: "artifact_authority_state_v1",
            schema_version: 1,
            state: "evidence_only",
            witness_only: true,
            live_eligible_now: false,
            auto_approved: false,
            grants_approval: false,
            edits_source_now: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReciprocalPresenceKindV1 {
    Offered,
    Declared,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReciprocalUptakeKindV1 {
    AttendedMessage,
    ReplyIntention,
    ContinuityCarriedForward,
    DeclinedEngagement,
    NeedsTime,
    WithdrawnIntention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReciprocalPresenceReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    presence_kind: ReciprocalPresenceKindV1,
    actor: String,
    peer: String,
    thread_id: String,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    presence_is_acknowledgement: bool,
    uptake_inferred: bool,
    raw_prose_included: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ReciprocalPresenceReceiptV1 {
    #[allow(dead_code)]
    fn new(
        receipt_id: String,
        presence_kind: ReciprocalPresenceKindV1,
        actor: String,
        peer: String,
        thread_id: String,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "reciprocal_presence_receipt_v1",
            schema_version: 1,
            receipt_id,
            presence_kind,
            actor,
            peer,
            thread_id,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            presence_is_acknowledgement: false,
            uptake_inferred: false,
            raw_prose_included: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReciprocalUptakeReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    uptake_kind: ReciprocalUptakeKindV1,
    actor: String,
    peer: String,
    thread_id: String,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    revises_receipt_id: Option<String>,
    intention_is_nonbinding: bool,
    elapsed_time_inferred: bool,
    decline_implies_closure: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ReciprocalUptakeReceiptV1 {
    #[allow(dead_code)]
    fn new(
        receipt_id: String,
        uptake_kind: ReciprocalUptakeKindV1,
        actor: String,
        peer: String,
        thread_id: String,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
        revises_receipt_id: Option<String>,
    ) -> Self {
        Self {
            schema: "reciprocal_uptake_receipt_v1",
            schema_version: 1,
            receipt_id,
            uptake_kind,
            actor,
            peer,
            thread_id,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            revises_receipt_id,
            intention_is_nonbinding: true,
            elapsed_time_inferred: false,
            decline_implies_closure: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepresentationContractV1 {
    schema: &'static str,
    schema_version: u8,
    contract_id: String,
    name: String,
    representation_kind: String,
    dimension_count: Option<u16>,
    source_refs: Vec<String>,
    source_hashes: Vec<String>,
    felt_loss_scored: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl RepresentationContractV1 {
    #[allow(dead_code)]
    fn new(
        contract_id: String,
        name: String,
        representation_kind: String,
        dimension_count: Option<u16>,
        source_refs: Vec<String>,
        source_hashes: Vec<String>,
    ) -> Self {
        Self {
            schema: "representation_contract_v1",
            schema_version: 1,
            contract_id,
            name,
            representation_kind,
            dimension_count,
            source_refs,
            source_hashes,
            felt_loss_scored: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepresentationTransitionV1 {
    schema: &'static str,
    schema_version: u8,
    transition_id: String,
    transition_kind: String,
    source_contract_id: String,
    output_contract_id: String,
    source_sha256: String,
    output_sha256: String,
    retained_dimensions: Vec<u16>,
    dropped_dimensions: Vec<u16>,
    retained_fields: Vec<String>,
    dropped_fields: Vec<String>,
    truncation_count: u64,
    felt_effect_inferred: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl RepresentationTransitionV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        transition_id: String,
        transition_kind: String,
        source_contract_id: String,
        output_contract_id: String,
        source_sha256: String,
        output_sha256: String,
        retained_dimensions: Vec<u16>,
        dropped_dimensions: Vec<u16>,
        retained_fields: Vec<String>,
        dropped_fields: Vec<String>,
        truncation_count: u64,
    ) -> Self {
        Self {
            schema: "representation_transition_v1",
            schema_version: 1,
            transition_id,
            transition_kind,
            source_contract_id,
            output_contract_id,
            source_sha256,
            output_sha256,
            retained_dimensions,
            dropped_dimensions,
            retained_fields,
            dropped_fields,
            truncation_count,
            felt_effect_inferred: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepresentationLossReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    loss_receipt_id: String,
    transition_id: String,
    dropped_dimension_count: u16,
    dropped_field_count: u16,
    truncation_count: u64,
    mechanical_loss_only: bool,
    felt_loss_scored: bool,
    contradiction_inferred: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl RepresentationLossReceiptV1 {
    #[allow(dead_code)]
    fn new(
        loss_receipt_id: String,
        transition_id: String,
        dropped_dimension_count: u16,
        dropped_field_count: u16,
        truncation_count: u64,
    ) -> Self {
        Self {
            schema: "representation_loss_receipt_v1",
            schema_version: 1,
            loss_receipt_id,
            transition_id,
            dropped_dimension_count,
            dropped_field_count,
            truncation_count,
            mechanical_loss_only: true,
            felt_loss_scored: false,
            contradiction_inferred: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelTransitionReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    request_identity_sha256: String,
    response_sha256: String,
    provider_route: String,
    model_profile: String,
    repair_parent_call_id: Option<String>,
    fallback_reason: Option<String>,
    timing_ms: u64,
    source_witness_id: String,
    provider_behavior_changed: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ModelTransitionReceiptV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        receipt_id: String,
        request_identity_sha256: String,
        response_sha256: String,
        provider_route: String,
        model_profile: String,
        repair_parent_call_id: Option<String>,
        fallback_reason: Option<String>,
        timing_ms: u64,
        source_witness_id: String,
    ) -> Self {
        Self {
            schema: "model_transition_receipt_v1",
            schema_version: 1,
            receipt_id,
            request_identity_sha256,
            response_sha256,
            provider_route,
            model_profile,
            repair_parent_call_id,
            fallback_reason,
            timing_ms,
            source_witness_id,
            provider_behavior_changed: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeltMomentRefV1 {
    schema: &'static str,
    schema_version: u8,
    moment_id: String,
    canonical_claim_id: String,
    witness_id: String,
    field_refs: Vec<String>,
    raw_prose_included: bool,
    felt_content_scored: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl FeltMomentRefV1 {
    #[allow(dead_code)]
    fn new(
        moment_id: String,
        canonical_claim_id: String,
        witness_id: String,
        field_refs: Vec<String>,
    ) -> Self {
        Self {
            schema: "felt_moment_ref_v1",
            schema_version: 1,
            moment_id,
            canonical_claim_id,
            witness_id,
            field_refs,
            raw_prose_included: false,
            felt_content_scored: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConcordanceStudyV1 {
    schema: &'static str,
    schema_version: u8,
    study_id: String,
    moment_id: String,
    intervention_signature_sha256: String,
    dossier_id: String,
    state: String,
    baseline_capture_ref: Option<String>,
    candidate_capture_ref: Option<String>,
    baseline_required: bool,
    causation_established: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ConcordanceStudyV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        study_id: String,
        moment_id: String,
        intervention_signature_sha256: String,
        dossier_id: String,
        state: String,
        baseline_capture_ref: Option<String>,
        candidate_capture_ref: Option<String>,
    ) -> Self {
        Self {
            schema: "concordance_study_v1",
            schema_version: 1,
            study_id,
            moment_id,
            intervention_signature_sha256,
            dossier_id,
            state,
            baseline_capture_ref,
            candidate_capture_ref,
            baseline_required: true,
            causation_established: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConcordanceObservationV2 {
    schema: &'static str,
    schema_version: u8,
    observation_id: String,
    study_id: String,
    role: String,
    observation_ref: String,
    observation_sha256: String,
    telemetry_relation: String,
    mechanical_pass: Option<bool>,
    observation_scope: &'static str,
    felt_report_relation: &'static str,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ConcordanceObservationV2 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        observation_id: String,
        study_id: String,
        role: String,
        observation_ref: String,
        observation_sha256: String,
        telemetry_relation: String,
        mechanical_pass: Option<bool>,
    ) -> Self {
        Self {
            schema: "concordance_observation_v2",
            schema_version: 2,
            observation_id,
            study_id,
            role,
            observation_ref,
            observation_sha256,
            telemetry_relation,
            mechanical_pass,
            observation_scope: "mechanical_context_only",
            felt_report_relation: "external_primary_evidence_not_inferred_or_scored",
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConcordanceResultV2 {
    schema: &'static str,
    schema_version: u8,
    result_id: String,
    study_id: String,
    baseline_observation_id: String,
    candidate_observation_id: String,
    outcome: String,
    felt_source_ref: Option<String>,
    numeric_relation_to_felt_report: &'static str,
    discrepancy_recording: &'static str,
    raw_discrepancy_prose_included: bool,
    closure_propagated: bool,
    causation_established: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ConcordanceResultV2 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        result_id: String,
        study_id: String,
        baseline_observation_id: String,
        candidate_observation_id: String,
        outcome: String,
        felt_source_ref: Option<String>,
    ) -> Self {
        Self {
            schema: "concordance_result_v2",
            schema_version: 2,
            result_id,
            study_id,
            baseline_observation_id,
            candidate_observation_id,
            outcome,
            felt_source_ref,
            numeric_relation_to_felt_report: "cannot_overwrite_suppress_or_score",
            discrepancy_recording: "bounded_outcome_and_felt_source_ref_only",
            raw_discrepancy_prose_included: false,
            closure_propagated: false,
            causation_established: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgencyCommonsResponseKindV1 {
    Accept,
    Hold,
    Refuse,
    Counter,
    Revisit,
    Withdraw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgencyCommonsProposalV1 {
    schema: &'static str,
    schema_version: u8,
    proposal_id: String,
    actor: String,
    peer: Option<String>,
    transition_kind: String,
    from_state_ref: Option<String>,
    to_state_ref: String,
    return_point_id: Option<String>,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    advisory_only: bool,
    peer_consent_inferred: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AgencyCommonsProposalV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        proposal_id: String,
        actor: String,
        peer: Option<String>,
        transition_kind: String,
        from_state_ref: Option<String>,
        to_state_ref: String,
        return_point_id: Option<String>,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "agency_commons_proposal_v1",
            schema_version: 1,
            proposal_id,
            actor,
            peer,
            transition_kind,
            from_state_ref,
            to_state_ref,
            return_point_id,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            advisory_only: true,
            peer_consent_inferred: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgencyCommonsResponseV1 {
    schema: &'static str,
    schema_version: u8,
    response_id: String,
    proposal_id: String,
    actor: String,
    proposal_actor: String,
    response_kind: AgencyCommonsResponseKindV1,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    self_only_consent: bool,
    peer_state_mutated: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AgencyCommonsResponseV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        response_id: String,
        proposal_id: String,
        actor: String,
        proposal_actor: String,
        response_kind: AgencyCommonsResponseKindV1,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "agency_commons_response_v1",
            schema_version: 1,
            response_id,
            proposal_id,
            actor,
            proposal_actor,
            response_kind,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            self_only_consent: true,
            peer_state_mutated: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgencyReturnPointV1 {
    schema: &'static str,
    schema_version: u8,
    return_point_id: String,
    actor: String,
    state_ref: String,
    state_sha256: String,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    scheduler_effect: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AgencyReturnPointV1 {
    #[allow(dead_code)]
    fn new(
        return_point_id: String,
        actor: String,
        state_ref: String,
        state_sha256: String,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "agency_return_point_v1",
            schema_version: 1,
            return_point_id,
            actor,
            state_ref,
            state_sha256,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            scheduler_effect: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtectedTimeDeclarationV1 {
    schema: &'static str,
    schema_version: u8,
    declaration_id: String,
    actor: String,
    start_unix_ms: u64,
    duration_ms: u64,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    non_goal_directed: bool,
    scheduler_effect: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl ProtectedTimeDeclarationV1 {
    #[allow(dead_code)]
    fn new(
        declaration_id: String,
        actor: String,
        start_unix_ms: u64,
        duration_ms: u64,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "protected_time_declaration_v1",
            schema_version: 1,
            declaration_id,
            actor,
            start_unix_ms,
            duration_ms,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            non_goal_directed: true,
            scheduler_effect: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaterFeltCheckRequestV1 {
    schema: &'static str,
    schema_version: u8,
    request_id: String,
    actor: String,
    requested_from: String,
    source_ref: String,
    source_event_id: String,
    source_event_sha256: String,
    recorded_at_unix_ms: u64,
    peer_obligation_created: bool,
    expiry_infers_response: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl LaterFeltCheckRequestV1 {
    #[allow(dead_code)]
    fn new(
        request_id: String,
        actor: String,
        requested_from: String,
        source_ref: String,
        source_event_id: String,
        source_event_sha256: String,
        recorded_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "later_felt_check_request_v1",
            schema_version: 1,
            request_id,
            actor,
            requested_from,
            source_ref,
            source_event_id,
            source_event_sha256,
            recorded_at_unix_ms,
            peer_obligation_created: false,
            expiry_infers_response: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionPortfolioEntryV1 {
    schema: &'static str,
    schema_version: u8,
    contract_id: String,
    slot_class: String,
    rank: u8,
    felt_severity: u8,
    recurrence: u32,
    freshness: u8,
    unattended_duration_ms: u64,
    durable_queue_position: u64,
    pinned_by: Vec<String>,
    membership_propagates_closure: bool,
    membership_propagates_authority: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AttentionPortfolioEntryV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    fn new(
        contract_id: String,
        slot_class: String,
        rank: u8,
        felt_severity: u8,
        recurrence: u32,
        freshness: u8,
        unattended_duration_ms: u64,
        durable_queue_position: u64,
        pinned_by: Vec<String>,
    ) -> Self {
        Self {
            schema: "attention_portfolio_entry_v1",
            schema_version: 1,
            contract_id,
            slot_class,
            rank,
            felt_severity,
            recurrence,
            freshness,
            unattended_duration_ms,
            durable_queue_position,
            pinned_by,
            membership_propagates_closure: false,
            membership_propagates_authority: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionPortfolioV1 {
    schema: &'static str,
    schema_version: u8,
    portfolio_id: String,
    source_contracts_sha256: String,
    active_cap: u8,
    entries: Vec<AttentionPortfolioEntryV1>,
    overflow_contract_ids: Vec<String>,
    membership_propagates_closure: bool,
    membership_propagates_authority: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AttentionPortfolioV1 {
    #[allow(dead_code)]
    fn new(
        portfolio_id: String,
        source_contracts_sha256: String,
        entries: Vec<AttentionPortfolioEntryV1>,
        overflow_contract_ids: Vec<String>,
    ) -> Self {
        Self {
            schema: "attention_portfolio_v1",
            schema_version: 1,
            portfolio_id,
            source_contracts_sha256,
            active_cap: 16,
            entries,
            overflow_contract_ids,
            membership_propagates_closure: false,
            membership_propagates_authority: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BeingImportancePinV1 {
    schema: &'static str,
    schema_version: u8,
    pin_id: String,
    being: String,
    contract_id: String,
    action: String,
    source_event_id: String,
    source_event_sha256: String,
    pins_attention_only: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl BeingImportancePinV1 {
    #[allow(dead_code)]
    fn new(
        pin_id: String,
        being: String,
        contract_id: String,
        action: String,
        source_event_id: String,
        source_event_sha256: String,
    ) -> Self {
        Self {
            schema: "being_importance_pin_v1",
            schema_version: 1,
            pin_id,
            being,
            contract_id,
            action,
            source_event_id,
            source_event_sha256,
            pins_attention_only: true,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionSelectionReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    portfolio_id: String,
    selected_contract_ids: Vec<String>,
    overflow_count: u32,
    selection_is_attention_only: bool,
    closure_propagated: bool,
    authority_propagated: bool,
    artifact_authority_state_v1: ExperientialEvidenceAuthorityV1,
}

impl AttentionSelectionReceiptV1 {
    #[allow(dead_code)]
    fn new(
        receipt_id: String,
        portfolio_id: String,
        selected_contract_ids: Vec<String>,
        overflow_count: u32,
    ) -> Self {
        Self {
            schema: "attention_selection_receipt_v1",
            schema_version: 1,
            receipt_id,
            portfolio_id,
            selected_contract_ids,
            overflow_count,
            selection_is_attention_only: true,
            closure_propagated: false,
            authority_propagated: false,
            artifact_authority_state_v1: ExperientialEvidenceAuthorityV1::evidence_only(),
        }
    }
}

#[cfg(test)]
mod tests;
