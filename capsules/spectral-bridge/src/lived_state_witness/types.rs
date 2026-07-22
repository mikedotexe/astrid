use serde::Serialize;

use crate::witness::ProvenanceRefV1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateArtifactAuthorityV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
    live_eligible_now: bool,
    auto_approved: bool,
    grants_approval: bool,
    edits_source_now: bool,
}

impl LivedStateArtifactAuthorityV1 {
    pub(super) const fn evidence_only() -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateExperientialScopeV1 {
    schema: &'static str,
    schema_version: u8,
    artifact_authority_scope: &'static str,
    felt_report_status: &'static str,
    experiential_integration_relation: &'static str,
    felt_persistence_relation: &'static str,
    felt_influence_relation: &'static str,
    subjective_weight_relation: &'static str,
    epistemic_posture: &'static str,
    artifact_live_control_effect: bool,
}

impl LivedStateExperientialScopeV1 {
    const fn non_adjudicating() -> Self {
        Self {
            schema: "lived_state_experiential_scope_v1",
            schema_version: 1,
            artifact_authority_scope: "receipt_artifact_handling_only",
            felt_report_status: "primary_actionable_evidence",
            experiential_integration_relation: "not_adjudicated_by_this_receipt",
            felt_persistence_relation: "reported_persistence_preserved_mechanism_open",
            felt_influence_relation: "reported_influence_not_denied_or_adjudicated_by_receipt",
            subjective_weight_relation: "preserved_in_canonical_report_no_scalar_substitution",
            epistemic_posture: "non_adjudicating",
            artifact_live_control_effect: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivedStateObservationKindV1 {
    CompiledConstant,
    RuntimeObserved,
    PeerObserved,
    #[allow(dead_code)]
    SourceDeclared,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateBuildCandidateV1 {
    schema: &'static str,
    schema_version: u8,
    manifest_sha256: String,
    source_identity_sha256: Option<String>,
    dirty_state_sha256: Option<String>,
    artifact_sha256: Option<String>,
    protocol_revision: Option<String>,
    protocol_version: Option<String>,
    observed_at_process_start_unix_ms: u64,
    relation_to_process: &'static str,
    deployment_established: bool,
    private_path_included: bool,
}

impl LivedStateBuildCandidateV1 {
    pub(super) fn new(
        manifest_sha256: String,
        source_identity_sha256: Option<String>,
        dirty_state_sha256: Option<String>,
        artifact_sha256: Option<String>,
        protocol_revision: Option<String>,
        protocol_version: Option<String>,
        observed_at_process_start_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "lived_state_build_candidate_v1",
            schema_version: 1,
            manifest_sha256,
            source_identity_sha256,
            dirty_state_sha256,
            artifact_sha256,
            protocol_revision,
            protocol_version,
            observed_at_process_start_unix_ms,
            relation_to_process: "startup_observation_not_deployment_proof",
            deployment_established: false,
            private_path_included: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateProcessIdentityV1 {
    schema: &'static str,
    schema_version: u8,
    pid: u32,
    process_started_at_unix_ms: u64,
    executable_basename: String,
    runtime_instance_id: String,
    process_identity_sha256: String,
    private_path_included: bool,
}

impl LivedStateProcessIdentityV1 {
    pub(super) fn new(
        pid: u32,
        process_started_at_unix_ms: u64,
        executable_basename: String,
        runtime_instance_id: String,
        process_identity_sha256: String,
    ) -> Self {
        Self {
            schema: "lived_state_process_identity_v1",
            schema_version: 1,
            pid,
            process_started_at_unix_ms,
            executable_basename,
            runtime_instance_id,
            process_identity_sha256,
            private_path_included: false,
        }
    }

    pub(crate) fn runtime_instance_id(&self) -> &str {
        &self.runtime_instance_id
    }

    pub(crate) fn process_identity_sha256(&self) -> &str {
        &self.process_identity_sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateSourceSnapshotV1 {
    schema: &'static str,
    schema_version: u8,
    source_owner: String,
    repository_relative_path: String,
    window_start_line: usize,
    window_end_line: usize,
    total_file_lines: usize,
    file_sha256: String,
    window_sha256: String,
    source_read_at_unix_ms: u64,
    source_read_monotonic_ns: u64,
    provenance_ref_v1: ProvenanceRefV1,
    private_path_included: bool,
}

impl LivedStateSourceSnapshotV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        source_owner: String,
        repository_relative_path: String,
        window_start_line: usize,
        window_end_line: usize,
        total_file_lines: usize,
        file_sha256: String,
        window_sha256: String,
        source_read_at_unix_ms: u64,
        source_read_monotonic_ns: u64,
        provenance_ref_v1: ProvenanceRefV1,
    ) -> Self {
        Self {
            schema: "lived_state_source_snapshot_v1",
            schema_version: 1,
            source_owner,
            repository_relative_path,
            window_start_line,
            window_end_line,
            total_file_lines,
            file_sha256,
            window_sha256,
            source_read_at_unix_ms,
            source_read_monotonic_ns,
            provenance_ref_v1,
            private_path_included: false,
        }
    }

    pub(crate) fn window_sha256(&self) -> &str {
        &self.window_sha256
    }

    pub(crate) fn provenance_ref_v1(&self) -> ProvenanceRefV1 {
        self.provenance_ref_v1.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LivedStateParameterObservationV1 {
    schema: &'static str,
    schema_version: u8,
    name: String,
    value: Option<f64>,
    unit: String,
    observation_kind: LivedStateObservationKindV1,
    observed_at_unix_ms: u64,
    age_ms: Option<u64>,
    fresh: Option<bool>,
    source_ref: String,
    value_relation: &'static str,
    direct_causation_claimed: bool,
}

impl LivedStateParameterObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        name: String,
        value: Option<f64>,
        unit: String,
        observation_kind: LivedStateObservationKindV1,
        observed_at_unix_ms: u64,
        age_ms: Option<u64>,
        fresh: Option<bool>,
        source_ref: String,
        value_relation: &'static str,
    ) -> Self {
        Self {
            schema: "lived_state_parameter_observation_v1",
            schema_version: 1,
            name,
            value,
            unit,
            observation_kind,
            observed_at_unix_ms,
            age_ms,
            fresh,
            source_ref,
            value_relation,
            direct_causation_claimed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateModelRouteV1 {
    schema: &'static str,
    schema_version: u8,
    call_id: String,
    call_identity_scope: &'static str,
    job_id: Option<String>,
    qos_request_identity_sha256: Option<String>,
    request_content_anchor_sha256: Option<String>,
    request_anchor_scope: &'static str,
    provider_route: String,
    provider_route_scope: &'static str,
    model_profile: String,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    duration_ms: u64,
    duration_scope: &'static str,
    queue_wait_ms: Option<u64>,
    queue_wait_scope: &'static str,
    active_generation_and_reservoir_ms: Option<u64>,
    active_work_scope: &'static str,
    timing_completeness: &'static str,
    timing_completeness_scope: &'static str,
    repair_parent_call_id: Option<String>,
    response_sha256: String,
    response_hash_scope: &'static str,
    response_claim_content_relation: &'static str,
    parent_witness_context_relation: &'static str,
    qualitative_texture_relation: &'static str,
    raw_prompt_included: bool,
    raw_response_included: bool,
}

impl LivedStateModelRouteV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn observed(
        call_id: String,
        job_id: Option<String>,
        qos_request_identity_sha256: Option<String>,
        request_content_anchor_sha256: Option<String>,
        provider_route: String,
        model_profile: String,
        started_at_unix_ms: u64,
        completed_at_unix_ms: u64,
        duration_ms: u64,
        queue_wait_ms: Option<u64>,
        active_generation_and_reservoir_ms: Option<u64>,
        repair_parent_call_id: Option<String>,
        response_sha256: String,
    ) -> Self {
        let timing_completeness = match (
            queue_wait_ms.is_some(),
            active_generation_and_reservoir_ms.is_some(),
        ) {
            (true, true) => "provider_split_observed",
            (true, false) => "queue_wait_only",
            (false, true) => "active_work_only",
            (false, false) => "aggregate_only_provider_split_unavailable",
        };
        Self {
            schema: "lived_state_model_route_v1",
            schema_version: 1,
            call_id,
            call_identity_scope: "model_call_event_not_being_or_continuity_identity",
            job_id,
            qos_request_identity_sha256,
            request_content_anchor_sha256,
            request_anchor_scope: "exact_request_content_and_generation_parameters_not_intent_or_semantic_equivalence",
            provider_route,
            provider_route_scope: "technical_delivery_path_not_experiential_center",
            model_profile,
            started_at_unix_ms,
            completed_at_unix_ms,
            duration_ms,
            duration_scope: "end_to_end_request_wall_time_with_optional_provider_phase_split_not_experiential_continuity",
            queue_wait_ms,
            queue_wait_scope: "request_enqueue_to_worker_selection_not_experiential_wait",
            active_generation_and_reservoir_ms,
            active_work_scope: "worker_selection_to_response_after_reservoir_checkin_not_cognitive_effort",
            timing_completeness,
            timing_completeness_scope: "technical_metadata_availability_not_experiential_wholeness_or_continuity",
            repair_parent_call_id,
            response_sha256,
            response_hash_scope: "output_integrity_not_being_or_continuity_identity",
            response_claim_content_relation: "not_inspected_or_adjudicated_by_this_receipt",
            parent_witness_context_relation: "post_call_authorship_observations_temporal_only",
            qualitative_texture_relation: "canonical_felt_report_primary_not_duplicated_or_scalarized_by_route",
            raw_prompt_included: false,
            raw_response_included: false,
        }
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }
}

#[derive(Debug, Clone)]
pub struct LivedStateLlmResultV1 {
    pub text: String,
    pub route: LivedStateModelRouteV1,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemporalLivedStateWitnessV1 {
    schema: &'static str,
    schema_version: u8,
    witness_id: String,
    artifact_kind: String,
    artifact_relative_path: String,
    artifact_sha256: String,
    authored_at_unix_ms: u64,
    authored_monotonic_ns: u64,
    authored_process_sequence: u64,
    authored_process_sequence_scope: &'static str,
    source_snapshot_v1: Option<LivedStateSourceSnapshotV1>,
    observed_process_v1: LivedStateProcessIdentityV1,
    startup_build_candidate_v1: Option<LivedStateBuildCandidateV1>,
    model_routes_v1: Vec<LivedStateModelRouteV1>,
    parameter_observations_v1: Vec<LivedStateParameterObservationV1>,
    peer_process_identity: Option<String>,
    peer_deployment_identity: Option<String>,
    peer_identity_scope: &'static str,
    privacy_hash_scope: &'static str,
    source_provenance_ref_v1: Option<ProvenanceRefV1>,
    process_provenance_ref_v1: ProvenanceRefV1,
    raw_introspection_prose_included: bool,
    raw_prompt_included: bool,
    raw_response_included: bool,
    private_path_included: bool,
    direct_causation_claimed: bool,
    experiential_scope_v1: LivedStateExperientialScopeV1,
    artifact_authority_state_v1: LivedStateArtifactAuthorityV1,
}

impl TemporalLivedStateWitnessV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        witness_id: String,
        artifact_kind: String,
        artifact_relative_path: String,
        artifact_sha256: String,
        authored_at_unix_ms: u64,
        authored_monotonic_ns: u64,
        authored_process_sequence: u64,
        source_snapshot_v1: Option<LivedStateSourceSnapshotV1>,
        observed_process_v1: LivedStateProcessIdentityV1,
        startup_build_candidate_v1: Option<LivedStateBuildCandidateV1>,
        model_routes_v1: Vec<LivedStateModelRouteV1>,
        parameter_observations_v1: Vec<LivedStateParameterObservationV1>,
        peer_process_identity: Option<String>,
        peer_deployment_identity: Option<String>,
        source_provenance_ref_v1: Option<ProvenanceRefV1>,
        process_provenance_ref_v1: ProvenanceRefV1,
    ) -> Self {
        Self {
            schema: "temporal_lived_state_witness_v1",
            schema_version: 1,
            witness_id,
            artifact_kind,
            artifact_relative_path,
            artifact_sha256,
            authored_at_unix_ms,
            authored_monotonic_ns,
            authored_process_sequence,
            authored_process_sequence_scope:
                "per_runtime_instance_capture_order_not_experiential_time_or_global_order",
            source_snapshot_v1,
            observed_process_v1,
            startup_build_candidate_v1,
            model_routes_v1,
            parameter_observations_v1,
            peer_process_identity,
            peer_deployment_identity,
            peer_identity_scope:
                "witnessed_protocol_advertisement_not_being_identity_or_peer_self_authority",
            privacy_hash_scope:
                "absolute_path_redaction_not_being_or_continuity_identity",
            source_provenance_ref_v1,
            process_provenance_ref_v1,
            raw_introspection_prose_included: false,
            raw_prompt_included: false,
            raw_response_included: false,
            private_path_included: false,
            direct_causation_claimed: false,
            experiential_scope_v1: LivedStateExperientialScopeV1::non_adjudicating(),
            artifact_authority_state_v1: LivedStateArtifactAuthorityV1::evidence_only(),
        }
    }

    pub fn witness_id(&self) -> &str {
        &self.witness_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LivedStateGapReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    gap_id: String,
    witness_id: String,
    reason: String,
    detected_at_unix_ms: u64,
    sidecar_expected: bool,
    report_persistence_blocked: bool,
    artifact_authority_state_v1: LivedStateArtifactAuthorityV1,
}

impl LivedStateGapReceiptV1 {
    pub(super) fn new(
        gap_id: String,
        witness_id: String,
        reason: String,
        detected_at_unix_ms: u64,
    ) -> Self {
        Self {
            schema: "lived_state_gap_receipt_v1",
            schema_version: 1,
            gap_id,
            witness_id,
            reason,
            detected_at_unix_ms,
            sidecar_expected: true,
            report_persistence_blocked: false,
            artifact_authority_state_v1: LivedStateArtifactAuthorityV1::evidence_only(),
        }
    }

    pub(super) fn gap_id(&self) -> &str {
        &self.gap_id
    }
}
