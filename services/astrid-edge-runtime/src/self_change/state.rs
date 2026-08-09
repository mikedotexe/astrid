use sha2::{Digest, Sha256};

use super::schema::{
    BuildEvidenceV1, BuildStateV1, CandidateLifecycleV1, CandidatePhaseV1, DOMAIN_STATE_SCHEMA_V1,
    ProbationEvidenceV1, ProbationStateV1, RollbackEvidenceV1, RollbackStateV1,
    ScheduledIntrospectionKindV1, SelfChangeCommandV1, SelfChangeDomainStateV1,
    SelfChangeReceiptDraftV1, TRANSITION_REQUEST_SCHEMA_V1, TransitionActorV1, TransitionRequestV1,
};
use super::validation::{
    canonical_sha256, sha256_hex, validate_bounded_slug, validate_candidate_patch,
    validate_exact_model_attestation, validate_exact_model_attestation_static,
    validate_instance_id, validate_prefixed_hex_id, validate_scheduled_introspection,
    validate_sha256_named,
};
use super::{SelfChangeError, SelfChangeResult};

#[derive(serde::Serialize)]
struct CandidateIdSeed<'a> {
    instance_id: &'a str,
    source_id: &'a str,
    patch_sha256: &'a str,
    response_sha256: &'a str,
}

#[must_use]
pub fn derive_candidate_id(
    instance_id: &str,
    source_id: &str,
    patch_sha256: &str,
    response_sha256: &str,
) -> String {
    let seed = CandidateIdSeed {
        instance_id,
        source_id,
        patch_sha256,
        response_sha256,
    };
    // These borrowed strings are infallibly serializable.
    let digest = serde_json::to_vec(&seed).unwrap_or_else(|_| {
        format!("{instance_id}\0{source_id}\0{patch_sha256}\0{response_sha256}").into_bytes()
    });
    let hash = format!("{:x}", Sha256::digest(digest));
    format!("sc-{}", &hash[..24])
}

/// Computes the canonical digest of one domain state.
///
/// # Errors
///
/// Returns an error when the state cannot be serialized canonically.
pub fn state_sha256(state: &SelfChangeDomainStateV1) -> SelfChangeResult<String> {
    canonical_sha256(state)
}

/// Applies one validated transition atomically to an in-memory state value.
///
/// The caller must persist the returned receipt before making any separately-authorized external
/// effect. This function performs no I/O and leaves `state` untouched on every error.
///
/// # Errors
///
/// Returns an error for malformed or stale input, invalid authority, replay,
/// an illegal lifecycle transition, overflow, or serialization failure.
pub fn apply_transition(
    state: &mut SelfChangeDomainStateV1,
    request: &TransitionRequestV1,
) -> SelfChangeResult<SelfChangeReceiptDraftV1> {
    validate_state(state)?;
    validate_request_shape(request)?;

    if state.consumed_command_ids.contains(&request.command_id) {
        return Err(SelfChangeError::ReplayCommand);
    }
    let authority_key = authority_replay_key(&request.actor)?;
    if state.consumed_authority_keys.contains(&authority_key) {
        return Err(SelfChangeError::ReplayAttestation);
    }
    if let TransitionActorV1::ExactModel { attestation } = &request.actor {
        let exact_key = exact_attestation_key(attestation);
        if state.consumed_attestation_keys.contains(&exact_key) {
            return Err(SelfChangeError::ReplayAttestation);
        }
    }

    let before_hash = state_sha256(state)?;
    if request.expected_state_sha256 != before_hash {
        return Err(SelfChangeError::StaleStateHash);
    }
    validate_actor(
        &request.actor,
        &state.instance_id,
        request.occurred_at_unix_ms,
    )?;

    let from_phase = state.active.as_ref().map(|candidate| candidate.phase);
    let mut next = state.clone();
    apply_command(&mut next, request, &before_hash)?;
    next.revision = next
        .revision
        .checked_add(1)
        .ok_or(SelfChangeError::ArithmeticOverflow)?;
    next.consumed_command_ids.insert(request.command_id.clone());
    next.consumed_authority_keys.insert(authority_key);
    if let TransitionActorV1::ExactModel { attestation } = &request.actor {
        next.consumed_attestation_keys
            .insert(exact_attestation_key(attestation));
    }
    validate_state(&next)?;
    let resulting_state_sha256 = state_sha256(&next)?;
    let to_phase = next.active.as_ref().map(|candidate| candidate.phase);

    let receipt = SelfChangeReceiptDraftV1 {
        command_id: request.command_id.clone(),
        candidate_id: request.candidate_id.clone(),
        occurred_at_unix_ms: request.occurred_at_unix_ms,
        actor: request.actor.clone(),
        command: request.command.kind(),
        from_phase,
        to_phase,
        expected_state_sha256: before_hash,
        resulting_state_sha256,
    };
    *state = next;
    Ok(receipt)
}

fn apply_command(
    state: &mut SelfChangeDomainStateV1,
    request: &TransitionRequestV1,
    before_hash: &str,
) -> SelfChangeResult<()> {
    match &request.command {
        SelfChangeCommandV1::Propose { patch } => {
            require_exact_model(&request.actor)?;
            if state.active.is_some() {
                return Err(SelfChangeError::ActiveTransactionExists);
            }
            if state
                .completed_candidate_ids
                .contains(&request.candidate_id)
            {
                return Err(SelfChangeError::ReplayCommand);
            }
            validate_candidate_patch(patch)?;
            if request.candidate_id != patch.candidate_id {
                return Err(SelfChangeError::CandidateMismatch);
            }
            let TransitionActorV1::ExactModel { attestation } = &request.actor else {
                return Err(SelfChangeError::InvalidAuthority);
            };
            let expected_candidate_id = derive_candidate_id(
                &state.instance_id,
                &patch.source_id,
                &patch.patch_sha256,
                &attestation.response_sha256,
            );
            if patch.candidate_id != expected_candidate_id {
                return Err(SelfChangeError::CandidateMismatch);
            }
            state.active = Some(CandidateLifecycleV1 {
                patch: patch.clone(),
                phase: CandidatePhaseV1::Proposed,
                proposal_attestation: attestation.as_ref().clone(),
                nomination_attestation: None,
                build: BuildStateV1::NotStarted,
                probation: ProbationStateV1::NotScheduled,
                rollback: RollbackStateV1::NotRequired,
                activation_receipt_sha256: None,
                created_at_unix_ms: request.occurred_at_unix_ms,
                updated_at_unix_ms: request.occurred_at_unix_ms,
            });
            Ok(())
        },
        SelfChangeCommandV1::Archive => {
            require_operator(&request.actor)?;
            let candidate = require_candidate(state, &request.candidate_id)?;
            if !candidate.phase.is_terminal() {
                return Err(SelfChangeError::InvalidTransition);
            }
            state
                .completed_candidate_ids
                .insert(request.candidate_id.clone());
            state.active = None;
            Ok(())
        },
        command => {
            let candidate = require_candidate(state, &request.candidate_id)?;
            if request.occurred_at_unix_ms < candidate.updated_at_unix_ms {
                return Err(SelfChangeError::InvalidTransition);
            }
            apply_candidate_command(candidate, command, &request.actor, request, before_hash)
        },
    }
}

// Keeping the transition matrix in one match makes forbidden phase/authority pairs reviewable.
#[allow(clippy::too_many_lines)]
fn apply_candidate_command(
    candidate: &mut CandidateLifecycleV1,
    command: &SelfChangeCommandV1,
    actor: &TransitionActorV1,
    request: &TransitionRequestV1,
    before_hash: &str,
) -> SelfChangeResult<()> {
    match command {
        SelfChangeCommandV1::ValidateSource { source_tree_sha256 } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::Proposed)?;
            validate_sha256_named(source_tree_sha256, "source tree")?;
            if source_tree_sha256 != &candidate.patch.source_manifest_sha256 {
                return Err(SelfChangeError::StaleStateHash);
            }
            candidate.phase = CandidatePhaseV1::SourceValidated;
        },
        SelfChangeCommandV1::StartBuild { build_id } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::SourceValidated)?;
            validate_prefixed_hex_id(build_id, "build", "build_id")?;
            candidate.build = BuildStateV1::Running {
                build_id: build_id.clone(),
                started_at_unix_ms: request.occurred_at_unix_ms,
            };
            candidate.phase = CandidatePhaseV1::Building;
        },
        SelfChangeCommandV1::CompleteBuild { evidence } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::Building)?;
            validate_build_evidence(evidence, candidate, request.occurred_at_unix_ms)?;
            let BuildStateV1::Running {
                build_id,
                started_at_unix_ms,
            } = &candidate.build
            else {
                return Err(SelfChangeError::InvalidTransition);
            };
            if &evidence.build_id != build_id || evidence.completed_at_unix_ms < *started_at_unix_ms
            {
                return Err(SelfChangeError::InvalidTransition);
            }
            if evidence.passed {
                candidate.build = BuildStateV1::Passed {
                    evidence: evidence.clone(),
                };
                candidate.phase = CandidatePhaseV1::BuildPassed;
            } else {
                candidate.build = BuildStateV1::Failed {
                    evidence: evidence.clone(),
                };
                candidate.phase = CandidatePhaseV1::BuildFailed;
            }
        },
        SelfChangeCommandV1::ScheduleProbation {
            probation_id,
            introspection,
        } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::BuildPassed)?;
            validate_prefixed_hex_id(probation_id, "prob", "probation_id")?;
            validate_scheduled_introspection(introspection)?;
            if introspection.candidate_id != candidate.patch.candidate_id
                || introspection.instance_id != candidate.proposal_attestation.instance_id
                || introspection.kind != ScheduledIntrospectionKindV1::ProbationCheckpoint
                || introspection.expected_candidate_state_sha256 != before_hash
                || introspection.originating_trace_id != candidate.proposal_attestation.trace_id
                || introspection.originating_turn_id != candidate.proposal_attestation.turn_id
                || introspection.originating_response_sha256
                    != candidate.proposal_attestation.response_sha256
            {
                return Err(SelfChangeError::InvalidSchedule(
                    "schedule does not bind the candidate and authored proposal",
                ));
            }
            candidate.probation = ProbationStateV1::Scheduled {
                probation_id: probation_id.clone(),
                introspection: introspection.clone(),
            };
            candidate.phase = CandidatePhaseV1::ProbationScheduled;
        },
        SelfChangeCommandV1::StartProbation { started_at_unix_ms } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::ProbationScheduled)?;
            if *started_at_unix_ms != request.occurred_at_unix_ms {
                return Err(SelfChangeError::InvalidTransition);
            }
            let ProbationStateV1::Scheduled {
                probation_id,
                introspection,
            } = &candidate.probation
            else {
                return Err(SelfChangeError::InvalidTransition);
            };
            candidate.probation = ProbationStateV1::Running {
                probation_id: probation_id.clone(),
                introspection: introspection.clone(),
                started_at_unix_ms: *started_at_unix_ms,
            };
            candidate.phase = CandidatePhaseV1::ProbationRunning;
        },
        SelfChangeCommandV1::CompleteProbation { evidence } => {
            require_broker(actor)?;
            require_phase(candidate, CandidatePhaseV1::ProbationRunning)?;
            validate_probation_evidence(evidence, candidate, request.occurred_at_unix_ms)?;
            let ProbationStateV1::Running {
                probation_id,
                introspection,
                started_at_unix_ms,
            } = &candidate.probation
            else {
                return Err(SelfChangeError::InvalidTransition);
            };
            if &evidence.probation_id != probation_id
                || evidence.started_at_unix_ms != *started_at_unix_ms
                || evidence.completed_at_unix_ms < *started_at_unix_ms
            {
                return Err(SelfChangeError::InvalidTransition);
            }
            if evidence.passed {
                candidate.probation = ProbationStateV1::Passed {
                    evidence: evidence.clone(),
                    introspection: introspection.clone(),
                };
                candidate.phase = CandidatePhaseV1::ProbationPassed;
            } else {
                candidate.probation = ProbationStateV1::Failed {
                    evidence: evidence.clone(),
                    introspection: introspection.clone(),
                };
                candidate.phase = CandidatePhaseV1::ProbationFailed;
            }
        },
        SelfChangeCommandV1::Nominate { rationale_sha256 } => {
            require_exact_model(actor)?;
            require_phase(candidate, CandidatePhaseV1::ProbationPassed)?;
            validate_sha256_named(rationale_sha256, "nomination rationale")?;
            let TransitionActorV1::ExactModel { attestation } = actor else {
                return Err(SelfChangeError::InvalidAuthority);
            };
            let completed_at = match &candidate.probation {
                ProbationStateV1::Passed { evidence, .. } => evidence.completed_at_unix_ms,
                _ => return Err(SelfChangeError::InvalidTransition),
            };
            if attestation.authored_at_unix_ms < completed_at
                || attestation.turn_id == candidate.proposal_attestation.turn_id
            {
                return Err(SelfChangeError::InvalidAttestation(
                    "nomination must be a later, distinct authored turn",
                ));
            }
            candidate.nomination_attestation = Some(attestation.as_ref().clone());
            candidate.phase = CandidatePhaseV1::PromotionNominated;
        },
        SelfChangeCommandV1::RecordActivation {
            deployment_receipt_sha256,
        } => {
            require_operator(actor)?;
            require_phase(candidate, CandidatePhaseV1::PromotionNominated)?;
            validate_sha256_named(deployment_receipt_sha256, "deployment receipt")?;
            candidate.activation_receipt_sha256 = Some(deployment_receipt_sha256.clone());
            candidate.phase = CandidatePhaseV1::Active;
        },
        SelfChangeCommandV1::RequestRollback {
            rollback_id,
            reason_sha256,
        } => {
            require_rollback_requester(actor)?;
            require_phase(candidate, CandidatePhaseV1::Active)?;
            validate_prefixed_hex_id(rollback_id, "rollback", "rollback_id")?;
            validate_sha256_named(reason_sha256, "rollback reason")?;
            candidate.rollback = RollbackStateV1::Pending {
                rollback_id: rollback_id.clone(),
                reason_sha256: reason_sha256.clone(),
                requested_at_unix_ms: request.occurred_at_unix_ms,
            };
            candidate.phase = CandidatePhaseV1::RollbackPending;
        },
        SelfChangeCommandV1::CompleteRollback { evidence } => {
            require_broker_or_operator(actor)?;
            require_phase(candidate, CandidatePhaseV1::RollbackPending)?;
            validate_rollback_evidence(evidence, candidate, request.occurred_at_unix_ms)?;
            candidate.rollback = RollbackStateV1::Completed {
                evidence: evidence.clone(),
            };
            candidate.phase = CandidatePhaseV1::RolledBack;
        },
        SelfChangeCommandV1::Cancel { reason_sha256 } => {
            require_exact_or_operator(actor)?;
            validate_sha256_named(reason_sha256, "cancellation reason")?;
            if candidate.phase == CandidatePhaseV1::Active
                || candidate.phase == CandidatePhaseV1::RollbackPending
                || candidate.phase.is_terminal()
            {
                return Err(SelfChangeError::InvalidTransition);
            }
            candidate.phase = CandidatePhaseV1::Cancelled;
        },
        SelfChangeCommandV1::Expire { reason_sha256 } => {
            require_scheduler(actor)?;
            validate_sha256_named(reason_sha256, "expiry reason")?;
            if candidate.phase == CandidatePhaseV1::Active
                || candidate.phase == CandidatePhaseV1::RollbackPending
                || candidate.phase.is_terminal()
            {
                return Err(SelfChangeError::InvalidTransition);
            }
            candidate.phase = CandidatePhaseV1::Expired;
        },
        SelfChangeCommandV1::Propose { .. } | SelfChangeCommandV1::Archive => {
            return Err(SelfChangeError::InvalidTransition);
        },
    }
    candidate.updated_at_unix_ms = request.occurred_at_unix_ms;
    Ok(())
}

fn validate_state(state: &SelfChangeDomainStateV1) -> SelfChangeResult<()> {
    if state.schema != DOMAIN_STATE_SCHEMA_V1 {
        return Err(SelfChangeError::InvalidSchema("self-change state"));
    }
    validate_instance_id(&state.instance_id)?;
    for candidate_id in &state.completed_candidate_ids {
        validate_prefixed_hex_id(candidate_id, "sc", "completed candidate_id")?;
    }
    for command_id in &state.consumed_command_ids {
        validate_prefixed_hex_id(command_id, "cmd", "consumed command_id")?;
    }
    for key in &state.consumed_attestation_keys {
        validate_sha256_named(key, "consumed attestation key")?;
    }
    for key in &state.consumed_authority_keys {
        validate_sha256_named(key, "consumed authority key")?;
    }
    if let Some(candidate) = &state.active {
        validate_candidate(state, candidate)?;
        if state
            .completed_candidate_ids
            .contains(&candidate.patch.candidate_id)
        {
            return Err(SelfChangeError::InvalidTransition);
        }
    }
    Ok(())
}

fn validate_candidate(
    state: &SelfChangeDomainStateV1,
    candidate: &CandidateLifecycleV1,
) -> SelfChangeResult<()> {
    validate_candidate_patch(&candidate.patch)?;
    validate_exact_model_attestation_static(&candidate.proposal_attestation, &state.instance_id)?;
    let expected_id = derive_candidate_id(
        &state.instance_id,
        &candidate.patch.source_id,
        &candidate.patch.patch_sha256,
        &candidate.proposal_attestation.response_sha256,
    );
    if candidate.patch.candidate_id != expected_id
        || candidate.created_at_unix_ms <= 0
        || candidate.updated_at_unix_ms < candidate.created_at_unix_ms
    {
        return Err(SelfChangeError::CandidateMismatch);
    }
    if let Some(attestation) = &candidate.nomination_attestation {
        validate_exact_model_attestation_static(attestation, &state.instance_id)?;
        if attestation.turn_id == candidate.proposal_attestation.turn_id {
            return Err(SelfChangeError::InvalidAttestation(
                "proposal and nomination turn must differ",
            ));
        }
    }
    validate_candidate_phase_consistency(candidate)
}

fn validate_candidate_phase_consistency(candidate: &CandidateLifecycleV1) -> SelfChangeResult<()> {
    let build_matches = match candidate.phase {
        CandidatePhaseV1::Proposed | CandidatePhaseV1::SourceValidated => {
            matches!(candidate.build, BuildStateV1::NotStarted)
        },
        CandidatePhaseV1::Building => matches!(candidate.build, BuildStateV1::Running { .. }),
        CandidatePhaseV1::BuildFailed => matches!(candidate.build, BuildStateV1::Failed { .. }),
        CandidatePhaseV1::Cancelled | CandidatePhaseV1::Expired => true,
        _ => matches!(candidate.build, BuildStateV1::Passed { .. }),
    };
    let probation_matches = match candidate.phase {
        CandidatePhaseV1::Proposed
        | CandidatePhaseV1::SourceValidated
        | CandidatePhaseV1::Building
        | CandidatePhaseV1::BuildPassed
        | CandidatePhaseV1::BuildFailed => {
            matches!(candidate.probation, ProbationStateV1::NotScheduled)
        },
        CandidatePhaseV1::ProbationScheduled => {
            matches!(candidate.probation, ProbationStateV1::Scheduled { .. })
        },
        CandidatePhaseV1::ProbationRunning => {
            matches!(candidate.probation, ProbationStateV1::Running { .. })
        },
        CandidatePhaseV1::ProbationFailed => {
            matches!(candidate.probation, ProbationStateV1::Failed { .. })
        },
        CandidatePhaseV1::ProbationPassed
        | CandidatePhaseV1::PromotionNominated
        | CandidatePhaseV1::Active
        | CandidatePhaseV1::RollbackPending
        | CandidatePhaseV1::RolledBack => {
            matches!(candidate.probation, ProbationStateV1::Passed { .. })
        },
        CandidatePhaseV1::Cancelled | CandidatePhaseV1::Expired => true,
    };
    let rollback_matches = match candidate.phase {
        CandidatePhaseV1::RollbackPending => {
            matches!(candidate.rollback, RollbackStateV1::Pending { .. })
        },
        CandidatePhaseV1::RolledBack => {
            matches!(candidate.rollback, RollbackStateV1::Completed { .. })
        },
        _ => matches!(candidate.rollback, RollbackStateV1::NotRequired),
    };
    let nomination_matches = matches!(
        candidate.phase,
        CandidatePhaseV1::Cancelled | CandidatePhaseV1::Expired
    ) || matches!(
        candidate.phase,
        CandidatePhaseV1::PromotionNominated
            | CandidatePhaseV1::Active
            | CandidatePhaseV1::RollbackPending
            | CandidatePhaseV1::RolledBack
    ) == candidate.nomination_attestation.is_some();
    let activation_matches = matches!(
        candidate.phase,
        CandidatePhaseV1::Active | CandidatePhaseV1::RollbackPending | CandidatePhaseV1::RolledBack
    ) == candidate.activation_receipt_sha256.is_some();
    if build_matches
        && probation_matches
        && rollback_matches
        && nomination_matches
        && activation_matches
    {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidTransition)
    }
}

fn validate_request_shape(request: &TransitionRequestV1) -> SelfChangeResult<()> {
    if request.schema != TRANSITION_REQUEST_SCHEMA_V1 {
        return Err(SelfChangeError::InvalidSchema("transition request"));
    }
    validate_prefixed_hex_id(&request.command_id, "cmd", "command_id")?;
    validate_prefixed_hex_id(&request.candidate_id, "sc", "candidate_id")?;
    validate_sha256_named(&request.expected_state_sha256, "expected state")?;
    if request.occurred_at_unix_ms <= 0 {
        return Err(SelfChangeError::InvalidTransition);
    }
    Ok(())
}

fn validate_actor(
    actor: &TransitionActorV1,
    instance_id: &str,
    occurred_at_unix_ms: i64,
) -> SelfChangeResult<()> {
    match actor {
        TransitionActorV1::ExactModel { attestation } => {
            validate_exact_model_attestation(attestation, instance_id, occurred_at_unix_ms)
        },
        TransitionActorV1::CandidateBroker {
            broker_id,
            signed_receipt_sha256,
        } => {
            validate_bounded_slug(broker_id, 3, 64, "broker_id")?;
            validate_sha256_named(signed_receipt_sha256, "broker receipt")
        },
        TransitionActorV1::Operator {
            authorization_id,
            signed_receipt_sha256,
        } => {
            validate_prefixed_hex_id(authorization_id, "auth", "authorization_id")?;
            validate_sha256_named(signed_receipt_sha256, "operator receipt")
        },
        TransitionActorV1::Scheduler { schedule_id } => {
            validate_prefixed_hex_id(schedule_id, "si", "schedule_id")
        },
        TransitionActorV1::SafetyMonitor {
            event_id,
            evidence_sha256,
        } => {
            validate_prefixed_hex_id(event_id, "safe", "safety event_id")?;
            validate_sha256_named(evidence_sha256, "safety evidence")
        },
    }
}

fn authority_replay_key(actor: &TransitionActorV1) -> SelfChangeResult<String> {
    canonical_sha256(actor)
}

fn exact_attestation_key(attestation: &super::schema::ExactModelAttestationV1) -> String {
    sha256_hex(attestation.replay_key())
}

fn validate_build_evidence(
    evidence: &BuildEvidenceV1,
    candidate: &CandidateLifecycleV1,
    occurred_at_unix_ms: i64,
) -> SelfChangeResult<()> {
    validate_prefixed_hex_id(&evidence.build_id, "build", "build_id")?;
    validate_sha256_named(&evidence.source_tree_sha256, "build source tree")?;
    validate_sha256_named(&evidence.test_manifest_sha256, "test manifest")?;
    validate_sha256_named(&evidence.evidence_sha256, "build evidence")?;
    if let Some(artifact) = &evidence.artifact_sha256 {
        validate_sha256_named(artifact, "build artifact")?;
    }
    if evidence.source_tree_sha256 != candidate.patch.source_manifest_sha256
        || evidence.completed_at_unix_ms != occurred_at_unix_ms
        || evidence.passed != evidence.artifact_sha256.is_some()
    {
        return Err(SelfChangeError::InvalidTransition);
    }
    Ok(())
}

fn validate_probation_evidence(
    evidence: &ProbationEvidenceV1,
    candidate: &CandidateLifecycleV1,
    occurred_at_unix_ms: i64,
) -> SelfChangeResult<()> {
    validate_prefixed_hex_id(&evidence.probation_id, "prob", "probation_id")?;
    validate_sha256_named(&evidence.artifact_sha256, "probation artifact")?;
    validate_sha256_named(
        &evidence.health_manifest_sha256,
        "probation health manifest",
    )?;
    let artifact = match &candidate.build {
        BuildStateV1::Passed { evidence } => evidence.artifact_sha256.as_deref(),
        _ => None,
    };
    if artifact != Some(evidence.artifact_sha256.as_str())
        || evidence.completed_at_unix_ms != occurred_at_unix_ms
        || evidence.expected_samples == 0
        || evidence.observed_samples > evidence.expected_samples
    {
        return Err(SelfChangeError::InvalidTransition);
    }
    if evidence.passed {
        let covered = u64::from(evidence.observed_samples)
            .checked_mul(100)
            .ok_or(SelfChangeError::ArithmeticOverflow)?;
        let required = u64::from(evidence.expected_samples)
            .checked_mul(90)
            .ok_or(SelfChangeError::ArithmeticOverflow)?;
        if covered < required {
            return Err(SelfChangeError::LimitExceeded("probation sample coverage"));
        }
    }
    Ok(())
}

fn validate_rollback_evidence(
    evidence: &RollbackEvidenceV1,
    candidate: &CandidateLifecycleV1,
    occurred_at_unix_ms: i64,
) -> SelfChangeResult<()> {
    validate_prefixed_hex_id(&evidence.rollback_id, "rollback", "rollback_id")?;
    validate_sha256_named(&evidence.restored_artifact_sha256, "restored artifact")?;
    validate_sha256_named(&evidence.health_manifest_sha256, "rollback health manifest")?;
    let RollbackStateV1::Pending {
        rollback_id,
        requested_at_unix_ms,
        ..
    } = &candidate.rollback
    else {
        return Err(SelfChangeError::InvalidTransition);
    };
    if &evidence.rollback_id != rollback_id
        || evidence.completed_at_unix_ms != occurred_at_unix_ms
        || evidence.completed_at_unix_ms < *requested_at_unix_ms
    {
        return Err(SelfChangeError::InvalidTransition);
    }
    Ok(())
}

fn require_candidate<'a>(
    state: &'a mut SelfChangeDomainStateV1,
    candidate_id: &str,
) -> SelfChangeResult<&'a mut CandidateLifecycleV1> {
    let candidate = state
        .active
        .as_mut()
        .ok_or(SelfChangeError::NoActiveTransaction)?;
    if candidate.patch.candidate_id == candidate_id {
        Ok(candidate)
    } else {
        Err(SelfChangeError::CandidateMismatch)
    }
}

fn require_phase(
    candidate: &CandidateLifecycleV1,
    expected: CandidatePhaseV1,
) -> SelfChangeResult<()> {
    if candidate.phase == expected {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidTransition)
    }
}

fn require_exact_model(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(actor, TransitionActorV1::ExactModel { .. }) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_broker(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(actor, TransitionActorV1::CandidateBroker { .. }) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_operator(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(actor, TransitionActorV1::Operator { .. }) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_scheduler(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(actor, TransitionActorV1::Scheduler { .. }) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_exact_or_operator(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(
        actor,
        TransitionActorV1::ExactModel { .. } | TransitionActorV1::Operator { .. }
    ) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_broker_or_operator(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(
        actor,
        TransitionActorV1::CandidateBroker { .. } | TransitionActorV1::Operator { .. }
    ) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}

fn require_rollback_requester(actor: &TransitionActorV1) -> SelfChangeResult<()> {
    if matches!(
        actor,
        TransitionActorV1::ExactModel { .. }
            | TransitionActorV1::Operator { .. }
            | TransitionActorV1::SafetyMonitor { .. }
    ) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidAuthority)
    }
}
