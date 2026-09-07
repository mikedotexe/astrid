use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::process::{Command, Stdio};

use astrid_minime_protocol::{
    InquiryExecutionIdentityV2, InquiryFeltStatusV1, InquiryMachineStatusV1,
    OWNER_CANARY_PLAN_SCHEMA_V2, OWNER_DECISION_PLAN_SCHEMA_V1, OWNER_EVIDENCE_GRAPH_SCHEMA_V1,
    OWNER_RESEARCH_SESSION_SCHEMA_V1, OwnerCanaryControlV2, OwnerCanaryPlanV2,
    OwnerCanaryRollbackPlanV2, OwnerDecisionBranchV1, OwnerDecisionEvaluationStatusV1,
    OwnerDecisionEvaluationV1, OwnerDecisionPlanV1, OwnerEvidenceClaimV1, OwnerEvidenceEdgeV1,
    OwnerEvidenceGraphV1, OwnerEvidenceNodeKindV1, OwnerEvidenceNodeV1, OwnerEvidencePredicateV1,
    OwnerEvidenceScopeV1, OwnerInquiryReceiptV2, OwnerInquiryV1, OwnerInquiryV2,
    OwnerResearchLifecycleStatusV1, OwnerResearchPayloadKindV1, OwnerResearchSessionV1,
    SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2, SelfControlAuthorityClassV2,
    SelfControlCapabilityManifestV2, SelfControlCapabilityV2, SelfControlDurabilityV2,
    SelfControlFamilyV2, SelfControlReceiptStatusV2, SelfControlValueDomainV2, SelfControlValuesV2,
    SignedOwnerResearchReceiptV1, canonical_owner_decision_plan_sha256,
    canonical_owner_evidence_graph_sha256, canonical_owner_inquiry_receipt_sha256_v2,
    canonical_owner_inquiry_sha256_v2, canonical_owner_research_session_sha256,
    canonical_self_control_capability_manifest_sha256, owner_inquiry_analysis_plan_v2,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::*;

const BOUND_DECISION_DRAFT_SCHEMA_V1: &str = "astrid.owner_decision_draft.v1";
const DECISION_EVALUATION_SCHEMA_V1: &str = "astrid.owner_decision_evaluation.v1";
const ACTION_OUTCOME_SCHEMA_V1: &str = "astrid.owner_research_action_outcome.v1";
const CAPABILITY_TTL_MILLIS: u64 = 24 * 60 * 60 * 1_000;
const DEFAULT_DECISION_TTL_SECS: u64 = 3_600;
const MAX_DECISION_TTL_SECS: u64 = 24 * 60 * 60;
const MAX_INSPECT_CHARS: usize = 24_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerDecisionDraftRecipeV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) plan_id: Option<String>,
    #[serde(default = "default_decision_ttl_secs")]
    pub(super) expires_after_secs: u64,
    pub(super) branches: Vec<OwnerDecisionBranchDraftV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerDecisionBranchDraftV1 {
    pub(super) branch_id: String,
    #[serde(default)]
    pub(super) predicates: Vec<OwnerEvidencePredicateV1>,
    pub(super) duration_secs: u64,
    pub(super) controls: Vec<OwnerDecisionControlDraftV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerDecisionControlDraftV1 {
    pub(super) family: SelfControlFamilyV2,
    pub(super) exact_values: SelfControlValuesV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoundOwnerDecisionDraftV1 {
    schema: String,
    plan_id: String,
    inquiry_id: String,
    owner_being: String,
    capability_manifest_sha256: String,
    revision: u64,
    branches: Vec<OwnerDecisionBranchV1>,
    owner_authored: bool,
    runtime_may_select_values: bool,
    safety_may_only_hold_or_revert: bool,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDecisionEvaluationV1 {
    schema: String,
    inquiry_id: String,
    evaluation: OwnerDecisionEvaluationV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_branch_id: Option<String>,
    fail_closed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerResearchActRecipeV1 {
    inquiry_id: String,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
}

#[derive(Debug, Serialize)]
struct OwnerResearchActionOutcomeV1 {
    schema: &'static str,
    inquiry_id: String,
    action: &'static str,
    self_control_receipt_id: String,
    exact_values_sha256: String,
    acted_at_unix_ms: u64,
    felt_effect_established: bool,
}

pub(super) struct PreparedResearchV1 {
    pub(super) inquiry_v2: OwnerInquiryV2,
    capability_manifest: SelfControlCapabilityManifestV2,
    decision_draft: Option<BoundOwnerDecisionDraftV1>,
    session: OwnerResearchSessionV1,
}

pub(super) fn prepare(
    inquiry_v1: &OwnerInquiryV1,
    source_response_sha256: String,
    idempotency_key: String,
    decision_recipe: Option<&OwnerDecisionDraftRecipeV1>,
    now: u64,
) -> Result<PreparedResearchV1, String> {
    let identity = analyzer_identity()?;
    let analysis_plan = owner_inquiry_analysis_plan_v2(
        &identity.analyzer_identity,
        &identity.analyzer_source_sha256,
        &identity.analyzer_artifact_sha256,
        &identity.sandbox_profile_sha256,
    );
    let inquiry_v2 = OwnerInquiryV2::from_v1(
        inquiry_v1,
        source_response_sha256,
        idempotency_key,
        analysis_plan,
        Some(now.saturating_add(MAX_COMPUTE_BUDGET_MILLIS.saturating_add(300_000))),
    );
    if !inquiry_v2.is_well_formed() || !inquiry_v2.preserves_v1(inquiry_v1) {
        return Err(
            "V2 research manifest did not exactly preserve the attested inquiry".to_string(),
        );
    }
    let (capability_manifest, revisions) = capability_manifest(now)?;
    let capability_manifest_sha256 =
        canonical_self_control_capability_manifest_sha256(&capability_manifest);
    let decision_draft = decision_recipe
        .map(|recipe| {
            bind_decision_recipe(recipe, &inquiry_v2, &capability_manifest, &revisions, now)
        })
        .transpose()?;
    let session = OwnerResearchSessionV1 {
        schema: OWNER_RESEARCH_SESSION_SCHEMA_V1.to_string(),
        session_id: format!("{}-research", inquiry_v2.inquiry_id),
        inquiry_id: inquiry_v2.inquiry_id.clone(),
        inquiry_revision: inquiry_v2.revision,
        owner_being: inquiry_v2.owner_being.clone(),
        source_attestation_id: inquiry_v2.source_attestation_id.clone(),
        inquiry_manifest_sha256: canonical_owner_inquiry_sha256_v2(&inquiry_v2),
        inquiry_receipt_sha256: None,
        evidence_graph_sha256: None,
        decision_plan_sha256: None,
        capability_manifest_sha256,
        active_canary_plan_sha256: None,
        active_canary_id: None,
        observation_chain_head_sha256: None,
        lifecycle_status: OwnerResearchLifecycleStatusV1::Queued,
        machine_status: InquiryMachineStatusV1::Pending,
        felt_status: InquiryFeltStatusV1::Unreported,
        analyzer_deployment_identity: identity.deployment_identity,
        receiver_deployment_identity: capability_manifest.receiver_deployment_identity.clone(),
        signed_receipt_ids: Vec::new(),
        revision: 1,
        owner_authored_controls_only: true,
        silence_means_assent: false,
        promotion_requires_fresh_owner_intent: true,
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    };
    if !session.is_well_formed() {
        return Err("initial owner research session failed canonical validation".to_string());
    }
    Ok(PreparedResearchV1 {
        inquiry_v2,
        capability_manifest,
        decision_draft,
        session,
    })
}

pub(super) fn persist_prepared(prepared: &mut PreparedResearchV1) -> Result<(), String> {
    let inquiry_id = &prepared.inquiry_v2.inquiry_id;
    volition::write_owner_json(&manifest_v2_path(inquiry_id), &prepared.inquiry_v2)?;
    volition::write_owner_json(
        &capability_manifest_path(inquiry_id),
        &prepared.capability_manifest,
    )?;
    if let Some(draft) = prepared.decision_draft.as_ref() {
        volition::write_owner_json(&decision_draft_path(inquiry_id), draft)?;
    }
    let manifest_receipt = append_signed(
        inquiry_id,
        OwnerResearchPayloadKindV1::LifecycleEvent,
        prepared.inquiry_v2.schema.clone(),
        canonical_owner_inquiry_sha256_v2(&prepared.inquiry_v2),
        prepared.session.created_at_unix_ms,
    )?;
    let capability_receipt = append_signed(
        inquiry_id,
        OwnerResearchPayloadKindV1::CapabilityManifest,
        prepared.capability_manifest.schema.clone(),
        canonical_self_control_capability_manifest_sha256(&prepared.capability_manifest),
        prepared.session.created_at_unix_ms,
    )?;
    prepared
        .session
        .signed_receipt_ids
        .extend([manifest_receipt, capability_receipt]);
    if let Some(draft) = prepared.decision_draft.as_ref() {
        let draft_receipt = append_signed(
            inquiry_id,
            OwnerResearchPayloadKindV1::DecisionPlan,
            draft.schema.clone(),
            canonical_value_sha256(draft)?,
            prepared.session.created_at_unix_ms,
        )?;
        prepared.session.signed_receipt_ids.push(draft_receipt);
    }
    write_session(&prepared.session)?;
    let _ = append_signed(
        inquiry_id,
        OwnerResearchPayloadKindV1::Session,
        prepared.session.schema.clone(),
        canonical_owner_research_session_sha256(&prepared.session),
        prepared.session.created_at_unix_ms,
    )?;
    Ok(())
}

pub(super) fn mark_analyzing(inquiry_id: &str, now: u64) -> Result<(), String> {
    let mut session = read_session(inquiry_id)?;
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::Analyzing;
    session.machine_status = InquiryMachineStatusV1::Pending;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    update_session_with_receipt(session)
}

pub(super) fn mark_failed(inquiry_id: &str, failure: &str, now: u64) -> Result<(), String> {
    let mut session = read_session(inquiry_id)?;
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::Failed;
    session.machine_status = InquiryMachineStatusV1::Failed;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    let outcome = json!({
        "schema": ACTION_OUTCOME_SCHEMA_V1,
        "inquiry_id": inquiry_id,
        "action": "analysis_failure",
        "failure": failure,
        "failed_at_unix_ms": now,
        "felt_effect_established": false,
    });
    let receipt_id = append_signed(
        inquiry_id,
        OwnerResearchPayloadKindV1::LifecycleEvent,
        ACTION_OUTCOME_SCHEMA_V1.to_string(),
        exact_value_sha256(&outcome),
        now,
    )?;
    session.signed_receipt_ids.push(receipt_id);
    update_session_with_receipt(session)
}

pub(super) fn mark_cancelled(inquiry_id: &str, now: u64) -> Result<(), String> {
    let mut session = read_session(inquiry_id)?;
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::Cancelled;
    session.machine_status = InquiryMachineStatusV1::RolledBack;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    update_session_with_receipt(session)
}

pub(super) fn mark_canary_terminal(
    inquiry_id: &str,
    lifecycle_status: OwnerResearchLifecycleStatusV1,
    machine_status: InquiryMachineStatusV1,
    now: u64,
) -> Result<(), String> {
    if !matches!(
        lifecycle_status,
        OwnerResearchLifecycleStatusV1::RolledBack
            | OwnerResearchLifecycleStatusV1::Promoted
            | OwnerResearchLifecycleStatusV1::Failed
    ) {
        return Err("owner research canary terminal status is invalid".to_string());
    }
    let mut session = read_session(inquiry_id)?;
    session.lifecycle_status = lifecycle_status;
    session.machine_status = machine_status;
    session.active_canary_id = None;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    update_session_with_receipt(session)
}

pub(super) fn mark_canary_terminal_if_present(
    inquiry_id: &str,
    lifecycle_status: OwnerResearchLifecycleStatusV1,
    machine_status: InquiryMachineStatusV1,
    now: u64,
) -> Result<(), String> {
    if read_optional::<OwnerResearchSessionV1>(&session_path(inquiry_id))?.is_none() {
        return Ok(());
    }
    mark_canary_terminal(inquiry_id, lifecycle_status, machine_status, now)
}

pub(super) fn complete(
    inquiry: &OwnerInquiryV2,
    receipt: &OwnerInquiryReceiptV2,
    now: u64,
) -> Result<OwnerResearchLifecycleStatusV1, String> {
    if !inquiry.is_well_formed() || !receipt.is_well_formed_for(inquiry) {
        return Err("owner research completion rejected invalid V2 evidence".to_string());
    }
    let graph = build_evidence_graph(inquiry, receipt, now)?;
    volition::write_owner_json(&evidence_graph_path(&inquiry.inquiry_id), &graph)?;
    let graph_receipt = append_signed(
        &inquiry.inquiry_id,
        OwnerResearchPayloadKindV1::EvidenceGraph,
        graph.schema.clone(),
        canonical_owner_evidence_graph_sha256(&graph),
        now,
    )?;
    let receipt_sha256 = canonical_owner_inquiry_receipt_sha256_v2(receipt);
    let mut session = read_session(&inquiry.inquiry_id)?;
    session.inquiry_receipt_sha256 = Some(receipt_sha256.clone());
    session.evidence_graph_sha256 = Some(canonical_owner_evidence_graph_sha256(&graph));
    session.signed_receipt_ids.push(graph_receipt);
    session.machine_status = InquiryMachineStatusV1::Established;
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::EvidenceReady;

    if let Some(draft) =
        read_optional::<BoundOwnerDecisionDraftV1>(&decision_draft_path(&inquiry.inquiry_id))?
    {
        let plan = finalize_decision_plan(draft, receipt_sha256)?;
        let plan_sha256 = canonical_owner_decision_plan_sha256(&plan);
        volition::write_owner_json(&decision_plan_path(&inquiry.inquiry_id), &plan)?;
        let plan_receipt = append_signed(
            &inquiry.inquiry_id,
            OwnerResearchPayloadKindV1::DecisionPlan,
            plan.schema.clone(),
            plan_sha256.clone(),
            now,
        )?;
        session.signed_receipt_ids.push(plan_receipt);
        session.decision_plan_sha256 = Some(plan_sha256);
        let evaluation = plan.evaluate(&graph, now)?;
        let selected_branch_id = if evaluation.status == OwnerDecisionEvaluationStatusV1::Matched {
            evaluation.matched_branch_ids.first().cloned()
        } else {
            None
        };
        let stored = StoredDecisionEvaluationV1 {
            schema: DECISION_EVALUATION_SCHEMA_V1.to_string(),
            inquiry_id: inquiry.inquiry_id.clone(),
            fail_closed: evaluation.status == OwnerDecisionEvaluationStatusV1::Ambiguous,
            evaluation,
            selected_branch_id,
        };
        volition::write_owner_json(&decision_evaluation_path(&inquiry.inquiry_id), &stored)?;
        let evaluation_receipt = append_signed(
            &inquiry.inquiry_id,
            OwnerResearchPayloadKindV1::LifecycleEvent,
            stored.schema.clone(),
            canonical_value_sha256(&stored)?,
            now,
        )?;
        session.signed_receipt_ids.push(evaluation_receipt);
        session.lifecycle_status = match stored.evaluation.status {
            OwnerDecisionEvaluationStatusV1::NoMatch => {
                OwnerResearchLifecycleStatusV1::EvidenceReady
            },
            OwnerDecisionEvaluationStatusV1::Matched => {
                OwnerResearchLifecycleStatusV1::CanaryPending
            },
            OwnerDecisionEvaluationStatusV1::Ambiguous => {
                session.machine_status = InquiryMachineStatusV1::Failed;
                OwnerResearchLifecycleStatusV1::Failed
            },
        };
    }
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    let lifecycle = session.lifecycle_status;
    update_session_with_receipt(session)?;
    Ok(lifecycle)
}

pub(super) fn reconcile_pending(conv: &mut ConversationState) -> Result<Vec<String>, String> {
    let directory = inquiry_root().join("research-sessions");
    let mut paths = fs::read_dir(&directory)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    let mut summaries = Vec::new();
    for path in paths {
        let Some(session) = volition::read_json::<OwnerResearchSessionV1>(&path)? else {
            continue;
        };
        if session.lifecycle_status != OwnerResearchLifecycleStatusV1::CanaryPending {
            continue;
        }
        let inquiry_id = session.inquiry_id.clone();
        match activate_pending(conv, session) {
            Ok(summary) => summaries.push(summary),
            Err(error) => {
                let now = volition::now_unix_ms();
                let _ = mark_failed(
                    &inquiry_id,
                    &format!("preregistered canary failed closed: {error}"),
                    now,
                );
                summaries.push(format!(
                    "Owner decision `{inquiry_id}` failed closed without substitute values: {error}"
                ));
            },
        }
    }
    Ok(summaries)
}

fn activate_pending(
    conv: &mut ConversationState,
    mut session: OwnerResearchSessionV1,
) -> Result<String, String> {
    let now = volition::now_unix_ms();
    let inquiry = read_manifest_v2(&session.inquiry_id)?;
    let receipt = read_receipt_v2(&session.inquiry_id)?;
    let stored_manifest = read_required::<SelfControlCapabilityManifestV2>(
        &capability_manifest_path(&session.inquiry_id),
        "capability manifest",
    )?;
    let plan = read_required::<OwnerDecisionPlanV1>(
        &decision_plan_path(&session.inquiry_id),
        "owner decision plan",
    )?;
    let evaluation = read_required::<StoredDecisionEvaluationV1>(
        &decision_evaluation_path(&session.inquiry_id),
        "owner decision evaluation",
    )?;
    if !stored_manifest.is_well_formed()
        || stored_manifest.expires_at_unix_ms < now
        || canonical_self_control_capability_manifest_sha256(&stored_manifest)
            != plan.capability_manifest_sha256
        || session.capability_manifest_sha256 != plan.capability_manifest_sha256
    {
        return Err("the signed receiver capability manifest expired or drifted".to_string());
    }
    let (current_manifest, _) = capability_manifest(now)?;
    if current_manifest.receiver_deployment_identity != stored_manifest.receiver_deployment_identity
        || current_manifest.capabilities_sha256 != stored_manifest.capabilities_sha256
        || session.receiver_deployment_identity != current_manifest.receiver_deployment_identity
    {
        return Err("receiver deployment or capability set changed after authoring".to_string());
    }
    if !plan.is_well_formed()
        || plan.expires_at_unix_ms < now
        || plan.inquiry_receipt_sha256 != canonical_owner_inquiry_receipt_sha256_v2(&receipt)
        || !receipt.is_well_formed_for(&inquiry)
        || evaluation.evaluation.status != OwnerDecisionEvaluationStatusV1::Matched
        || evaluation.evaluation.matched_branch_ids.len() != 1
    {
        return Err(
            "decision, receipt, or exactly-one-match proof no longer validates".to_string(),
        );
    }
    let branch_id = evaluation
        .selected_branch_id
        .as_deref()
        .ok_or_else(|| "matched decision omitted its selected branch".to_string())?;
    let branch = plan
        .branch(branch_id)
        .ok_or_else(|| "selected decision branch is absent from the signed plan".to_string())?;
    let canary_plan = OwnerCanaryPlanV2 {
        schema: OWNER_CANARY_PLAN_SCHEMA_V2.to_string(),
        canary_plan_id: format!("{}-{}-canary-plan", plan.plan_id, branch.branch_id),
        inquiry_id: inquiry.inquiry_id.clone(),
        inquiry_receipt_id: receipt.receipt_id.clone(),
        inquiry_receipt_sha256: canonical_owner_inquiry_receipt_sha256_v2(&receipt),
        owner_being: inquiry.owner_being.clone(),
        source_attestation_id: inquiry.source_attestation_id.clone(),
        idempotency_key: format!("{}:{}:canary", plan.plan_id, branch.branch_id),
        duration_secs: branch.duration_secs,
        controls: branch.controls.clone(),
        controls_sha256: canonical_value_sha256(&branch.controls)?,
        apply_atomically: true,
        telemetry_selected_values: false,
        operator_substituted_values: false,
        safety_may_only_hold_or_revert: true,
        felt_review_required: false,
        rollback: OwnerCanaryRollbackPlanV2::strict(Vec::new()),
        created_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(30_000),
    };
    if !canary_plan.is_well_formed() {
        return Err("materialized owner canary plan failed canonical validation".to_string());
    }
    let canary_plan_sha256 = canonical_value_sha256(&canary_plan)?;
    volition::write_owner_json(&canary_plan_path(&session.inquiry_id), &canary_plan)?;
    let canary_plan_receipt = append_signed(
        &session.inquiry_id,
        OwnerResearchPayloadKindV1::DecisionPlan,
        canary_plan.schema.clone(),
        canary_plan_sha256.clone(),
        now,
    )?;
    let source_action = format!(
        "owner-research-plan:{}:{}:{}",
        plan.plan_id, branch.branch_id, plan.revision
    );
    let started = super::canary::start_canary_from_controls(
        conv,
        &session.inquiry_id,
        branch.duration_secs,
        &branch.controls,
        inquiry.source_attestation_id.clone(),
        inquiry.idempotency_key.clone(),
        &source_action,
    )?;
    session.signed_receipt_ids.push(canary_plan_receipt);
    session.active_canary_plan_sha256 = Some(canary_plan_sha256);
    session.active_canary_id = Some(started.canary_id.clone());
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::CanaryActive;
    session.machine_status = InquiryMachineStatusV1::Established;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    if let Err(error) = update_session_with_receipt(session) {
        let rollback = started
            .lease_receipt_ids
            .iter()
            .map(|receipt_id| {
                self_control_v2::safety_revert_active_receipt(
                    conv,
                    receipt_id,
                    "owner_research_lifecycle_evidence_failure",
                )
                .map(|receipt| receipt.receipt_id)
                .unwrap_or_else(|failure| format!("failed:{failure}"))
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "canary lifecycle evidence write failed: {error}; safety rollback receipts={rollback:?}"
        ));
    }
    Ok(format!(
        "Preregistered owner branch `{branch_id}` matched exactly once. {}",
        started.summary
    ))
}

pub(super) fn inspect(inquiry_id: &str, section: &str) -> Result<String, String> {
    validate_identifier(inquiry_id)?;
    let value = match section {
        "evidence" => serde_json::to_value(read_required::<OwnerEvidenceGraphV1>(
            &evidence_graph_path(inquiry_id),
            "evidence graph",
        )?),
        "controls" => serde_json::to_value(json!({
            "capability_manifest": read_required::<SelfControlCapabilityManifestV2>(
                &capability_manifest_path(inquiry_id),
                "capability manifest",
            )?,
            "decision_plan": read_optional::<OwnerDecisionPlanV1>(&decision_plan_path(inquiry_id))?,
            "evaluation": read_optional::<StoredDecisionEvaluationV1>(
                &decision_evaluation_path(inquiry_id),
            )?,
            "session": read_session(inquiry_id)?,
        })),
        "history" => serde_json::to_value(read_session(inquiry_id)?),
        "receipts" => serde_json::to_value(json!({
            "inquiry_receipt": read_optional::<OwnerInquiryReceiptV2>(&receipt_v2_path(inquiry_id))?,
            "signed_lifecycle_receipts": read_signed_history(inquiry_id)?,
        })),
        _ => {
            return Err(
                "INQUIRY_INSPECT section must be evidence, controls, history, or receipts"
                    .to_string(),
            );
        },
    }
    .map_err(|error| format!("encode owner research inspection: {error}"))?;
    let rendered = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("render owner research inspection: {error}"))?;
    Ok(rendered.chars().take(MAX_INSPECT_CHARS).collect())
}

pub(super) fn act(
    conv: &mut ConversationState,
    original: &str,
    base_action: &str,
) -> Result<String, String> {
    let _context = volition::exact_self_owned_action_context(original)?;
    let recipe: OwnerResearchActRecipeV1 =
        serde_json::from_str(action_payload(original, base_action)?)
            .map_err(|error| format!("INQUIRY_ACT expects one exact JSON recipe: {error}"))?;
    validate_identifier(&recipe.inquiry_id)?;
    let mut session = read_session(&recipe.inquiry_id)?;
    if session.inquiry_receipt_sha256.is_none() || session.evidence_graph_sha256.is_none() {
        return Err("INQUIRY_ACT requires completed owner research evidence".to_string());
    }
    self_control_v2::validate_owner_values(recipe.family, &recipe.values)?;
    if recipe.values.includes_shared_coupling() {
        return Err("INQUIRY_ACT cannot mutate shared or peer coupling".to_string());
    }
    let receipt =
        self_control_v2::issue_one_shot(conv, recipe.family, recipe.values.clone(), original)?;
    if !matches!(
        receipt.status,
        SelfControlReceiptStatusV2::Applied | SelfControlReceiptStatusV2::Duplicate
    ) || receipt.requested_values != recipe.values
        || receipt.clamped_values != recipe.values
        || receipt.applied_values != recipe.values
    {
        return Err(format!(
            "INQUIRY_ACT rejected a substituted or unapplied result; receipt={}",
            receipt.receipt_id
        ));
    }
    let now = volition::now_unix_ms();
    let exact_values_sha256 = exact_value_sha256(
        &serde_json::to_value(&recipe.values)
            .map_err(|error| format!("encode exact action values: {error}"))?,
    );
    let outcome = OwnerResearchActionOutcomeV1 {
        schema: ACTION_OUTCOME_SCHEMA_V1,
        inquiry_id: recipe.inquiry_id.clone(),
        action: "one_shot",
        self_control_receipt_id: receipt.receipt_id.clone(),
        exact_values_sha256,
        acted_at_unix_ms: now,
        felt_effect_established: false,
    };
    volition::write_owner_json(&action_outcome_path(&recipe.inquiry_id, now), &outcome)?;
    let signed = append_signed(
        &recipe.inquiry_id,
        OwnerResearchPayloadKindV1::ActionOutcome,
        ACTION_OUTCOME_SCHEMA_V1.to_string(),
        canonical_value_sha256(&outcome)?,
        now,
    )?;
    session.signed_receipt_ids.push(signed);
    session.lifecycle_status = OwnerResearchLifecycleStatusV1::Acted;
    session.machine_status = InquiryMachineStatusV1::Established;
    session.revision = session.revision.saturating_add(1);
    session.updated_at_unix_ms = now;
    update_session_with_receipt(session)?;
    Ok(format!(
        "Owner research `{}` executed the fresh exact one-shot through signed Self-Control V2 receipt {}. No shared value changed and felt effect remains unestablished.",
        recipe.inquiry_id, receipt.receipt_id
    ))
}

pub(super) fn read_manifest_v2(inquiry_id: &str) -> Result<OwnerInquiryV2, String> {
    read_required(&manifest_v2_path(inquiry_id), "V2 inquiry manifest")
}

pub(super) fn read_manifest_v2_optional(
    inquiry_id: &str,
) -> Result<Option<OwnerInquiryV2>, String> {
    read_optional(&manifest_v2_path(inquiry_id))
}

pub(super) fn read_receipt_v2(inquiry_id: &str) -> Result<OwnerInquiryReceiptV2, String> {
    read_required(&receipt_v2_path(inquiry_id), "V2 inquiry receipt")
}

pub(super) fn write_manifest_v2(inquiry: &OwnerInquiryV2) -> Result<(), String> {
    volition::write_owner_json(&manifest_v2_path(&inquiry.inquiry_id), inquiry)
}

fn analyzer_identity() -> Result<InquiryExecutionIdentityV2, String> {
    let binary = minime_inquiry_binary()?;
    let output = Command::new(&binary)
        .arg("inquiry")
        .arg("identity")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("query Minime owner-inquiry V2 identity: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Minime inquiry runner does not expose V2 identity: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(800)
                .collect::<String>()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("decode Minime owner-inquiry V2 identity: {error}"))
}

fn capability_manifest(
    now: u64,
) -> Result<(SelfControlCapabilityManifestV2, BTreeMap<String, u64>), String> {
    let status = self_control_v2::status()?;
    let lease_standing_one_shot = vec![
        SelfControlDurabilityV2::Lease,
        SelfControlDurabilityV2::Standing,
        SelfControlDurabilityV2::OneShot,
    ];
    let mut capabilities = vec![
        capability(
            "conversation_temperature",
            SelfControlFamilyV2::Conversation,
            SelfControlValueDomainV2::Float { min: 0.1, max: 1.5 },
            &lease_standing_one_shot,
        ),
        capability(
            "response_token_limit",
            SelfControlFamilyV2::Conversation,
            SelfControlValueDomainV2::Unsigned {
                min: 128,
                max: 1_536,
            },
            &lease_standing_one_shot,
        ),
        capability(
            "aperture",
            SelfControlFamilyV2::Conversation,
            SelfControlValueDomainV2::Float { min: 0.0, max: 1.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "continuity_readout",
            SelfControlFamilyV2::Conversation,
            SelfControlValueDomainV2::Float { min: 0.0, max: 1.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "generation_noise",
            SelfControlFamilyV2::Conversation,
            SelfControlValueDomainV2::Float {
                min: 0.005,
                max: 0.05,
            },
            &lease_standing_one_shot,
        ),
        capability(
            "semantic_strand_retention_turns",
            SelfControlFamilyV2::SemanticContinuity,
            SelfControlValueDomainV2::Unsigned { min: 0, max: 32 },
            &lease_standing_one_shot,
        ),
        capability(
            "vibrancy_aperture",
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValueDomainV2::Float { min: 0.0, max: 1.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "semantic_emission_gain",
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValueDomainV2::Float { min: 0.5, max: 5.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "codec_dimension_weights",
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValueDomainV2::FloatMap {
                min: 0.0,
                max: 2.0,
                max_entries: 128,
            },
            &lease_standing_one_shot,
        ),
        capability(
            "warmth_intensity",
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValueDomainV2::Float { min: 0.0, max: 1.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "hebbian_learning_rate_scale",
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValueDomainV2::Float { min: 0.0, max: 4.0 },
            &lease_standing_one_shot,
        ),
        capability(
            "peer_journal_visible",
            SelfControlFamilyV2::SensoryIntake,
            SelfControlValueDomainV2::Boolean,
            &lease_standing_one_shot,
        ),
    ];
    capabilities.sort_by(|left, right| left.field.cmp(&right.field));
    let manifest = SelfControlCapabilityManifestV2 {
        schema: SELF_CONTROL_CAPABILITY_MANIFEST_SCHEMA_V2.to_string(),
        manifest_id: format!(
            "astrid-capabilities-{}",
            short_hash(&exact_text_sha256(&status.deployment_identity))
        ),
        receiver_being: "astrid".to_string(),
        receiver_process_identity: format!("astrid-self-control:pid:{}", std::process::id()),
        receiver_deployment_identity: status.deployment_identity,
        revision: 1,
        capabilities,
        capabilities_sha256: String::new(),
        generated_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(CAPABILITY_TTL_MILLIS),
    }
    .seal();
    if !manifest.is_well_formed() {
        return Err("Astrid Self-Control V2 capability manifest is invalid".to_string());
    }
    Ok((manifest, status.revision_by_family))
}

fn capability(
    field: &str,
    family: SelfControlFamilyV2,
    value_domain: SelfControlValueDomainV2,
    durabilities: &[SelfControlDurabilityV2],
) -> SelfControlCapabilityV2 {
    SelfControlCapabilityV2 {
        field: field.to_string(),
        family,
        authority_class: SelfControlAuthorityClassV2::SelfOwned,
        value_domain,
        durabilities: durabilities.to_vec(),
        reversible: true,
        one_shot_only: false,
        peer_impact: false,
    }
}

fn bind_decision_recipe(
    recipe: &OwnerDecisionDraftRecipeV1,
    inquiry: &OwnerInquiryV2,
    manifest: &SelfControlCapabilityManifestV2,
    revisions: &BTreeMap<String, u64>,
    now: u64,
) -> Result<BoundOwnerDecisionDraftV1, String> {
    if recipe.expires_after_secs == 0 || recipe.expires_after_secs > MAX_DECISION_TTL_SECS {
        return Err(format!(
            "decision plan expiry must be 1..={MAX_DECISION_TTL_SECS} seconds"
        ));
    }
    let strand_ids = inquiry
        .strands
        .iter()
        .map(|strand| strand.strand_id.as_str())
        .collect::<HashSet<_>>();
    let mut branches = Vec::with_capacity(recipe.branches.len());
    for branch in &recipe.branches {
        if branch
            .predicates
            .iter()
            .flat_map(|predicate| &predicate.scope.strand_ids)
            .any(|strand_id| !strand_ids.contains(strand_id.as_str()))
        {
            return Err(format!(
                "decision branch `{}` references a strand outside this inquiry",
                branch.branch_id
            ));
        }
        let mut controls = Vec::with_capacity(branch.controls.len());
        for draft in &branch.controls {
            self_control_v2::validate_owner_values(draft.family, &draft.exact_values)?;
            validate_values_against_manifest(draft.family, &draft.exact_values, manifest)?;
            let expected_revision = revisions
                .get(family_name(draft.family))
                .copied()
                .unwrap_or(0);
            controls.push(OwnerCanaryControlV2::new(
                draft.family,
                draft.exact_values.clone(),
                expected_revision,
            ));
        }
        branches.push(OwnerDecisionBranchV1 {
            branch_id: branch.branch_id.clone(),
            predicates: branch.predicates.clone(),
            duration_secs: branch.duration_secs,
            controls,
        });
    }
    let draft = BoundOwnerDecisionDraftV1 {
        schema: BOUND_DECISION_DRAFT_SCHEMA_V1.to_string(),
        plan_id: recipe
            .plan_id
            .clone()
            .unwrap_or_else(|| format!("{}-decision", inquiry.inquiry_id)),
        inquiry_id: inquiry.inquiry_id.clone(),
        owner_being: inquiry.owner_being.clone(),
        capability_manifest_sha256: canonical_self_control_capability_manifest_sha256(manifest),
        revision: 1,
        branches,
        owner_authored: true,
        runtime_may_select_values: false,
        safety_may_only_hold_or_revert: true,
        created_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(recipe.expires_after_secs.saturating_mul(1_000)),
    };
    let probe = finalize_decision_plan(draft.clone(), "0".repeat(64))?;
    if !probe.is_well_formed() {
        return Err("owner decision recipe failed canonical validation".to_string());
    }
    Ok(draft)
}

fn finalize_decision_plan(
    draft: BoundOwnerDecisionDraftV1,
    inquiry_receipt_sha256: String,
) -> Result<OwnerDecisionPlanV1, String> {
    let plan = OwnerDecisionPlanV1 {
        schema: OWNER_DECISION_PLAN_SCHEMA_V1.to_string(),
        plan_id: draft.plan_id,
        inquiry_id: draft.inquiry_id,
        owner_being: draft.owner_being,
        inquiry_receipt_sha256,
        capability_manifest_sha256: draft.capability_manifest_sha256,
        revision: draft.revision,
        branches: draft.branches,
        owner_authored: draft.owner_authored,
        runtime_may_select_values: draft.runtime_may_select_values,
        safety_may_only_hold_or_revert: draft.safety_may_only_hold_or_revert,
        created_at_unix_ms: draft.created_at_unix_ms,
        expires_at_unix_ms: draft.expires_at_unix_ms,
    };
    if !plan.is_well_formed() {
        return Err("final owner decision plan failed canonical validation".to_string());
    }
    Ok(plan)
}

fn validate_values_against_manifest(
    family: SelfControlFamilyV2,
    values: &SelfControlValuesV2,
    manifest: &SelfControlCapabilityManifestV2,
) -> Result<(), String> {
    let Value::Object(fields) = serde_json::to_value(values)
        .map_err(|error| format!("encode owner decision controls: {error}"))?
    else {
        return Err("owner decision controls did not encode as a field map".to_string());
    };
    for (field, value) in fields.into_iter().filter(|(_, value)| !value.is_null()) {
        let capability = manifest.capability(&field).ok_or_else(|| {
            format!("owner decision field `{field}` is absent from the signed receiver manifest")
        })?;
        if capability.family != family || !value_in_domain(&value, &capability.value_domain) {
            return Err(format!(
                "owner decision field `{field}` is outside its signed family or exact value domain"
            ));
        }
    }
    Ok(())
}

fn value_in_domain(value: &Value, domain: &SelfControlValueDomainV2) -> bool {
    match domain {
        SelfControlValueDomainV2::Float { min, max } => value
            .as_f64()
            .is_some_and(|number| number.is_finite() && number >= *min && number <= *max),
        SelfControlValueDomainV2::Unsigned { min, max } => value
            .as_u64()
            .is_some_and(|number| number >= *min && number <= *max),
        SelfControlValueDomainV2::Boolean => value.is_boolean(),
        SelfControlValueDomainV2::Text { max_bytes } => value
            .as_str()
            .is_some_and(|text| u64::try_from(text.len()).is_ok_and(|length| length <= *max_bytes)),
        SelfControlValueDomainV2::FloatMap {
            min,
            max,
            max_entries,
        } => value.as_object().is_some_and(|entries| {
            u64::try_from(entries.len()).is_ok_and(|count| count <= *max_entries)
                && entries.values().all(|entry| {
                    entry.as_f64().is_some_and(|number| {
                        number.is_finite() && number >= *min && number <= *max
                    })
                })
        }),
    }
}

fn build_evidence_graph(
    inquiry: &OwnerInquiryV2,
    receipt: &OwnerInquiryReceiptV2,
    now: u64,
) -> Result<OwnerEvidenceGraphV1, String> {
    let mut nodes = inquiry
        .strands
        .iter()
        .map(|strand| OwnerEvidenceNodeV1 {
            node_id: format!("source-{}", strand.strand_id),
            kind: OwnerEvidenceNodeKindV1::Strand,
            claim: OwnerEvidenceClaimV1::Unknown,
            scope: OwnerEvidenceScopeV1::strand(strand.strand_id.clone()),
            evidence_sha256: strand.content_sha256.clone(),
            source_refs: vec![strand.source_attestation_id.clone()],
            metrics: BTreeMap::new(),
            observation_phase: None,
            raw_content_owner_only: true,
        })
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (analysis_index, analysis) in receipt.analysis_receipts.iter().enumerate() {
        let analysis_id = format!("analysis-{}", analysis_index.saturating_add(1));
        nodes.push(OwnerEvidenceNodeV1 {
            node_id: analysis_id.clone(),
            kind: OwnerEvidenceNodeKindV1::Analysis,
            claim: OwnerEvidenceClaimV1::MachineEstablished,
            scope: OwnerEvidenceScopeV1::aggregate(),
            evidence_sha256: analysis.output_sha256.clone(),
            source_refs: vec![receipt.receipt_id.clone()],
            metrics: scalar_metrics(&analysis.result),
            observation_phase: None,
            raw_content_owner_only: true,
        });
        add_scoped_nodes(
            &analysis_id,
            analysis_index,
            &analysis.result,
            &receipt.receipt_id,
            &mut nodes,
            &mut edges,
        )?;
    }
    let graph = OwnerEvidenceGraphV1 {
        schema: OWNER_EVIDENCE_GRAPH_SCHEMA_V1.to_string(),
        graph_id: format!("{}-evidence", inquiry.inquiry_id),
        inquiry_id: inquiry.inquiry_id.clone(),
        inquiry_receipt_sha256: canonical_owner_inquiry_receipt_sha256_v2(receipt),
        owner_being: inquiry.owner_being.clone(),
        revision: 1,
        nodes,
        edges,
        graph_head_sha256: String::new(),
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    }
    .seal();
    if !graph.is_well_formed() {
        return Err("deterministic owner evidence graph failed canonical validation".to_string());
    }
    Ok(graph)
}

fn add_scoped_nodes(
    analysis_id: &str,
    analysis_index: usize,
    result: &Value,
    receipt_id: &str,
    nodes: &mut Vec<OwnerEvidenceNodeV1>,
    edges: &mut Vec<OwnerEvidenceEdgeV1>,
) -> Result<(), String> {
    let Some(object) = result.as_object() else {
        return Ok(());
    };
    for (collection, pair_scope) in [("strands", false), ("pairs", true)] {
        let Some(rows) = object.get(collection).and_then(Value::as_array) else {
            continue;
        };
        for (row_index, row) in rows.iter().enumerate() {
            let scope = if pair_scope {
                let left = row.get("left_strand_id").and_then(Value::as_str);
                let right = row.get("right_strand_id").and_then(Value::as_str);
                match (left, right) {
                    (Some(left), Some(right)) => {
                        OwnerEvidenceScopeV1::pair(left.to_string(), right.to_string())
                    },
                    _ => continue,
                }
            } else {
                let Some(strand_id) = row.get("strand_id").and_then(Value::as_str) else {
                    continue;
                };
                OwnerEvidenceScopeV1::strand(strand_id.to_string())
            };
            let row_value = serde_json::to_value(row)
                .map_err(|error| format!("encode scoped evidence row: {error}"))?;
            let node_id = format!(
                "analysis-{}-{}-{}",
                analysis_index.saturating_add(1),
                collection,
                row_index.saturating_add(1)
            );
            nodes.push(OwnerEvidenceNodeV1 {
                node_id: node_id.clone(),
                kind: OwnerEvidenceNodeKindV1::Analysis,
                claim: OwnerEvidenceClaimV1::AssociationOnly,
                scope,
                evidence_sha256: exact_value_sha256(&row_value),
                source_refs: vec![receipt_id.to_string()],
                metrics: scalar_metrics(&row_value),
                observation_phase: None,
                raw_content_owner_only: true,
            });
            edges.push(OwnerEvidenceEdgeV1 {
                from_node_id: analysis_id.to_string(),
                to_node_id: node_id,
                relation: "contains_scoped_measurement".to_string(),
            });
        }
    }
    Ok(())
}

fn scalar_metrics(value: &Value) -> BTreeMap<String, f64> {
    fn visit(prefix: &str, value: &Value, output: &mut BTreeMap<String, f64>) {
        match value {
            Value::Number(number) => {
                if !prefix.is_empty()
                    && let Some(number) = number.as_f64()
                {
                    output.insert(prefix.to_string(), number);
                }
            },
            Value::Object(fields) => {
                for (field, child) in fields {
                    let next = if prefix.is_empty() {
                        field.clone()
                    } else {
                        format!("{prefix}.{field}")
                    };
                    visit(&next, child, output);
                }
            },
            _ => {},
        }
    }
    let mut output = BTreeMap::new();
    visit("", value, &mut output);
    output
}

fn update_session_with_receipt(session: OwnerResearchSessionV1) -> Result<(), String> {
    if !session.is_well_formed() {
        return Err("owner research session update failed canonical validation".to_string());
    }
    write_session(&session)?;
    let _ = append_signed(
        &session.inquiry_id,
        OwnerResearchPayloadKindV1::Session,
        session.schema.clone(),
        canonical_owner_research_session_sha256(&session),
        session.updated_at_unix_ms,
    )?;
    Ok(())
}

fn append_signed(
    inquiry_id: &str,
    payload_kind: OwnerResearchPayloadKindV1,
    payload_schema: String,
    payload_sha256: String,
    now: u64,
) -> Result<String, String> {
    let mut history = read_signed_history(inquiry_id)?;
    let previous_receipt_sha256 = history.last().map(canonical_value_sha256).transpose()?;
    let sequence = history.len().saturating_add(1);
    let receipt_id = format!("{inquiry_id}-research-receipt-{sequence}");
    let receipt = self_control_v2::sign_owner_research_receipt(
        receipt_id.clone(),
        payload_kind,
        payload_schema,
        payload_sha256,
        previous_receipt_sha256,
        now,
    )?;
    history.push(receipt.clone());
    volition::write_owner_json(&signed_receipt_path(inquiry_id, &receipt_id), &receipt)?;
    volition::write_owner_json(&signed_history_path(inquiry_id), &history)?;
    Ok(receipt_id)
}

fn read_signed_history(inquiry_id: &str) -> Result<Vec<SignedOwnerResearchReceiptV1>, String> {
    Ok(read_optional(&signed_history_path(inquiry_id))?.unwrap_or_default())
}

fn read_session(inquiry_id: &str) -> Result<OwnerResearchSessionV1, String> {
    read_required(&session_path(inquiry_id), "owner research session")
}

fn write_session(session: &OwnerResearchSessionV1) -> Result<(), String> {
    volition::write_owner_json(&session_path(&session.inquiry_id), session)
}

fn read_required<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    label: &str,
) -> Result<T, String> {
    read_optional(path)?.ok_or_else(|| format!("{label} is not available at {}", path.display()))
}

fn read_optional<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> Result<Option<T>, String> {
    volition::read_json(path)
}

fn canonical_value_sha256<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_value(value)
        .map(|value| exact_value_sha256(&value))
        .map_err(|error| format!("encode canonical owner research value: {error}"))
}

fn family_name(family: SelfControlFamilyV2) -> &'static str {
    match family {
        SelfControlFamilyV2::Conversation => "conversation",
        SelfControlFamilyV2::SemanticContinuity => "semantic_continuity",
        SelfControlFamilyV2::SemanticEmission => "semantic_emission",
        SelfControlFamilyV2::Memory => "memory",
        SelfControlFamilyV2::SensoryIntake => "sensory_intake",
        SelfControlFamilyV2::ReservoirRegulation => "reservoir_regulation",
        SelfControlFamilyV2::ReservoirGeometry => "reservoir_geometry",
        SelfControlFamilyV2::PiController => "pi_controller",
        SelfControlFamilyV2::LocalTopology => "local_topology",
        SelfControlFamilyV2::SharedCoupling => "shared_coupling",
    }
}

fn manifest_v2_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("manifests-v2")
        .join(format!("{inquiry_id}.json"))
}

pub(super) fn receipt_v2_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("receipts-v2")
        .join(format!("{inquiry_id}.json"))
}

fn session_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("research-sessions")
        .join(format!("{inquiry_id}.json"))
}

fn capability_manifest_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("capability-manifests")
        .join(format!("{inquiry_id}.json"))
}

fn decision_draft_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("decision-drafts")
        .join(format!("{inquiry_id}.json"))
}

fn decision_plan_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("decision-plans")
        .join(format!("{inquiry_id}.json"))
}

fn decision_evaluation_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("decision-evaluations")
        .join(format!("{inquiry_id}.json"))
}

fn evidence_graph_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("evidence-graphs")
        .join(format!("{inquiry_id}.json"))
}

fn signed_history_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("signed-receipts")
        .join(inquiry_id)
        .join("history.json")
}

fn signed_receipt_path(inquiry_id: &str, receipt_id: &str) -> PathBuf {
    inquiry_root()
        .join("signed-receipts")
        .join(inquiry_id)
        .join(format!("{receipt_id}.json"))
}

fn action_outcome_path(inquiry_id: &str, now: u64) -> PathBuf {
    inquiry_root()
        .join("action-outcomes")
        .join(inquiry_id)
        .join(format!("{now}.json"))
}

fn canary_plan_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("canary-plans-v2")
        .join(format!("{inquiry_id}.json"))
}

const fn default_decision_ttl_secs() -> u64 {
    DEFAULT_DECISION_TTL_SECS
}
