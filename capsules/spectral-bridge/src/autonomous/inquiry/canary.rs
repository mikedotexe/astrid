use std::collections::HashSet;

use astrid_minime_protocol::{
    INQUIRY_OBSERVATION_SCHEMA_V1, InquiryFeltStatusV1, InquiryMachineStatusV1,
    InquiryObservationPhaseV1, InquiryObservationV1, InquiryRollbackStateV1, OwnerCanaryControlV2,
    OwnerInquiryStatusV1, SelfControlFamilyV2, SelfControlReceiptStatusV2, SelfControlReceiptV2,
    SelfControlValuesV2,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::*;

const INQUIRY_CANARY_SCHEMA_V1: &str = "astrid.owner_inquiry_canary.v1";
const DEFAULT_CANARY_DURATION_SECS: u64 = 600;
const MIN_CANARY_DURATION_SECS: u64 = 30;
const MAX_CANARY_DURATION_SECS: u64 = 900;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InquiryCanaryRecipeV1 {
    inquiry_id: String,
    #[serde(default = "default_canary_duration")]
    duration_secs: u64,
    values: AstridCanaryValuesV1,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AstridCanaryValuesV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    semantic_strand_retention_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversation_temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_token_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aperture: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continuity_readout: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vibrancy_aperture: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    semantic_emission_gain: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanaryFamilyV1 {
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    lease_intent_id: String,
    lease_receipt_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    standing_intent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    standing_receipt_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InquiryCanaryV1 {
    schema: String,
    canary_id: String,
    inquiry_id: String,
    owner_being: String,
    source_attestation_id: String,
    source_intent_id: String,
    duration_secs: u64,
    started_at_unix_ms: u64,
    sample_one_due_at_unix_ms: u64,
    sample_two_due_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    families: Vec<CanaryFamilyV1>,
    status: OwnerInquiryStatusV1,
    rollback_state: InquiryRollbackStateV1,
    observations: Vec<InquiryObservationV1>,
    felt_status: InquiryFeltStatusV1,
    silence_means_assent: bool,
    telemetry_selected_values: bool,
    operator_substituted_values: bool,
    updated_at_unix_ms: u64,
}

pub(super) struct StartedCanaryV1 {
    pub(super) canary_id: String,
    pub(super) lease_receipt_ids: Vec<String>,
    pub(super) summary: String,
}

pub(super) fn start_canary(
    conv: &mut ConversationState,
    original: &str,
    base_action: &str,
) -> Result<String, String> {
    let context = volition::exact_self_owned_action_context(original)?;
    let recipe: InquiryCanaryRecipeV1 =
        serde_json::from_str(action_payload(original, base_action)?)
            .map_err(|error| format!("INQUIRY_CANARY expects one exact JSON recipe: {error}"))?;
    validate_identifier(&recipe.inquiry_id)?;
    if !(MIN_CANARY_DURATION_SECS..=MAX_CANARY_DURATION_SECS).contains(&recipe.duration_secs) {
        return Err(format!(
            "canary duration must be {MIN_CANARY_DURATION_SECS}..={MAX_CANARY_DURATION_SECS} seconds; the value was rejected without substitution"
        ));
    }
    validate_canary_values(&recipe.values)?;
    let groups = canary_value_groups(&recipe.values);
    let exact_owner_selected_values = serde_json::to_value(&recipe.values)
        .map_err(|error| format!("encode exact canary values: {error}"))?;
    start_canary_groups(
        conv,
        &recipe.inquiry_id,
        recipe.duration_secs,
        groups,
        exact_owner_selected_values,
        context.source_attestation_id,
        context.intent_id,
        original,
    )
    .map(|started| started.summary)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_canary_from_controls(
    conv: &mut ConversationState,
    inquiry_id: &str,
    duration_secs: u64,
    controls: &[OwnerCanaryControlV2],
    source_attestation_id: String,
    source_intent_id: String,
    source_action: &str,
) -> Result<StartedCanaryV1, String> {
    validate_identifier(inquiry_id)?;
    if !(MIN_CANARY_DURATION_SECS..=MAX_CANARY_DURATION_SECS).contains(&duration_secs) {
        return Err(format!(
            "canary duration must be {MIN_CANARY_DURATION_SECS}..={MAX_CANARY_DURATION_SECS} seconds"
        ));
    }
    let status = self_control_v2::status()?;
    let mut families = Vec::new();
    let mut groups = Vec::with_capacity(controls.len());
    for control in controls {
        if !control.is_well_formed() || families.contains(&control.family) {
            return Err(
                "owner decision canary controls are malformed or duplicate a family".to_string(),
            );
        }
        families.push(control.family);
        let observed_revision = status
            .revision_by_family
            .get(canary_family_name(control.family))
            .copied()
            .unwrap_or(0);
        if observed_revision != control.expected_revision {
            return Err(format!(
                "owner decision expected {:?} revision {} but observed {}; no control was applied",
                control.family, control.expected_revision, observed_revision
            ));
        }
        self_control_v2::validate_owner_values(control.family, &control.exact_values)?;
        groups.push((control.family, control.exact_values.clone()));
    }
    if groups.is_empty() {
        return Err("owner decision canary requires at least one exact control".to_string());
    }
    let exact_owner_selected_values = serde_json::to_value(controls)
        .map_err(|error| format!("encode owner decision canary controls: {error}"))?;
    start_canary_groups(
        conv,
        inquiry_id,
        duration_secs,
        groups,
        exact_owner_selected_values,
        source_attestation_id,
        source_intent_id,
        source_action,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_canary_groups(
    conv: &mut ConversationState,
    inquiry_id: &str,
    duration_secs: u64,
    groups: Vec<(SelfControlFamilyV2, SelfControlValuesV2)>,
    exact_owner_selected_values: Value,
    source_attestation_id: String,
    source_intent_id: String,
    source_action: &str,
) -> Result<StartedCanaryV1, String> {
    if concern_queue::owner_inquiry_status(inquiry_id)? != Some(BeingConcernStatusV1::Completed) {
        return Err("INQUIRY_CANARY requires a completed deterministic inquiry".to_string());
    }
    let inquiry = read_manifest(inquiry_id)?;
    let mut receipt = read_receipt(inquiry_id)?;
    if !receipt.is_well_formed_for(&inquiry) {
        return Err("inquiry receipt failed integrity validation".to_string());
    }
    let mut runtime = read_runtime(inquiry_id)?;
    if runtime.active_canary_id.is_some() {
        return Err(
            "this inquiry already has an active canary; withdraw it before selecting another"
                .to_string(),
        );
    }
    let now = volition::now_unix_ms();
    let mut issued = Vec::<(
        SelfControlFamilyV2,
        SelfControlValuesV2,
        SelfControlReceiptV2,
    )>::new();
    for (family, values) in groups {
        self_control_v2::validate_owner_values(family, &values)?;
        match self_control_v2::issue_lease(
            conv,
            family,
            values.clone(),
            duration_secs,
            source_action,
        ) {
            Ok(applied)
                if matches!(
                    applied.status,
                    SelfControlReceiptStatusV2::Applied | SelfControlReceiptStatusV2::Duplicate
                ) && applied.requested_values == values
                    && applied.clamped_values == values
                    && applied.applied_values == values =>
            {
                issued.push((family, values, applied));
            },
            Ok(substituted) => {
                let mut rollback_set = issued.clone();
                if matches!(
                    substituted.status,
                    SelfControlReceiptStatusV2::Applied | SelfControlReceiptStatusV2::Duplicate
                ) {
                    rollback_set.push((family, values, substituted.clone()));
                }
                let rollback = rollback_issued(conv, &rollback_set, source_action);
                return Err(format!(
                    "Self-Control V2 did not preserve the exact selected values; the canary was rejected and prior families were rolled back: receipt={} rollback={rollback}",
                    substituted.receipt_id
                ));
            },
            Err(error) => {
                let rollback = rollback_issued(conv, &issued, source_action);
                return Err(format!(
                    "atomic canary application failed: {error}; prior families rollback={rollback}"
                ));
            },
        }
    }
    let expires_at = issued
        .iter()
        .filter_map(|(_, _, receipt)| receipt.control_expires_at_unix_ms)
        .min()
        .ok_or_else(|| {
            let rollback = rollback_issued(conv, &issued, source_action);
            format!("canary receipts omitted lease expiry; rollback={rollback}")
        })?;
    let canary_id = format!(
        "{}-canary-{}-{now}",
        inquiry_id,
        short_hash(&exact_text_sha256(source_action))
    );
    let receipt_ids = issued
        .iter()
        .map(|(_, _, receipt)| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let baseline = observation(
        inquiry_id,
        InquiryObservationPhaseV1::Baseline,
        now,
        receipt_ids.clone(),
        json!({
            "source": "signed_self_control_v2_receipts",
            "previous_values_by_family": issued.iter().map(|(family, _, receipt)| json!({
                "family": family,
                "previous_values": receipt.previous_values,
            })).collect::<Vec<_>>(),
            "exact_owner_selected_values": exact_owner_selected_values,
            "duration_secs": duration_secs,
            "expires_at_unix_ms": expires_at,
            "telemetry_selected_values": false,
            "operator_substituted_values": false,
            "felt_effect_established": false,
        }),
        InquiryMachineStatusV1::Established,
    );
    let families = issued
        .iter()
        .map(|(family, values, receipt)| CanaryFamilyV1 {
            family: *family,
            values: values.clone(),
            lease_intent_id: receipt.intent_id.clone(),
            lease_receipt_id: receipt.receipt_id.clone(),
            standing_intent_id: None,
            standing_receipt_id: None,
        })
        .collect::<Vec<_>>();
    let elapsed = expires_at.saturating_sub(now);
    let mut canary = InquiryCanaryV1 {
        schema: INQUIRY_CANARY_SCHEMA_V1.to_string(),
        canary_id: canary_id.clone(),
        inquiry_id: inquiry_id.to_string(),
        owner_being: "astrid".to_string(),
        source_attestation_id,
        source_intent_id,
        duration_secs,
        started_at_unix_ms: now,
        sample_one_due_at_unix_ms: now.saturating_add(elapsed / 3),
        sample_two_due_at_unix_ms: now.saturating_add(elapsed.saturating_mul(2) / 3),
        expires_at_unix_ms: expires_at,
        families,
        status: OwnerInquiryStatusV1::CanaryActive,
        rollback_state: InquiryRollbackStateV1::Scheduled,
        observations: vec![baseline.clone()],
        felt_status: InquiryFeltStatusV1::Unreported,
        silence_means_assent: false,
        telemetry_selected_values: false,
        operator_substituted_values: false,
        updated_at_unix_ms: now,
    };
    let setup_result = (|| {
        write_canary(&canary)?;
        receipt.observations.push(baseline);
        receipt.rollback_state = InquiryRollbackStateV1::Scheduled;
        receipt.machine_status = InquiryMachineStatusV1::Established;
        write_receipt(&receipt)?;
        runtime.status = OwnerInquiryStatusV1::CanaryActive;
        runtime.active_canary_id = Some(canary_id.clone());
        runtime.updated_at_unix_ms = now;
        write_runtime(&runtime)
    })();
    if let Err(error) = setup_result {
        let rollback = rollback_issued(conv, &issued, source_action);
        let failed_at = volition::now_unix_ms();
        let failure = observation(
            inquiry_id,
            InquiryObservationPhaseV1::PostRollback,
            failed_at,
            receipt_ids.clone(),
            json!({
                "setup_error": error.clone(),
                "rollback_attempt": rollback.clone(),
                "machine_status_only": true,
                "felt_effect_established": false,
            }),
            InquiryMachineStatusV1::Failed,
        );
        canary.status = OwnerInquiryStatusV1::Failed;
        canary.rollback_state = InquiryRollbackStateV1::Failed;
        canary.observations.push(failure.clone());
        canary.updated_at_unix_ms = failed_at;
        let _ = write_canary(&canary);
        receipt.observations.push(failure);
        receipt.rollback_state = InquiryRollbackStateV1::Failed;
        receipt.machine_status = InquiryMachineStatusV1::Failed;
        let _ = write_receipt(&receipt);
        runtime.status = OwnerInquiryStatusV1::Failed;
        runtime.active_canary_id = None;
        runtime.failure = Some(format!("canary setup evidence failed: {error}"));
        runtime.updated_at_unix_ms = failed_at;
        let _ = write_runtime(&runtime);
        return Err(format!(
            "canary setup evidence failed after control issue; every issued family was sent through rollback: {rollback}"
        ));
    }
    let summary = format!(
        "Canary `{canary_id}` applied the exact owner-selected values atomically through {} signed Self-Control V2 receipts. It expires at {expires_at}; two bounded machine samples are scheduled, felt review remains optional, and silence rolls every family back.",
        receipt_ids.len()
    );
    Ok(StartedCanaryV1 {
        canary_id,
        lease_receipt_ids: receipt_ids,
        summary,
    })
}

pub(super) fn withdraw_canary(
    conv: &mut ConversationState,
    original: &str,
    base_action: &str,
) -> Result<String, String> {
    let context = volition::exact_self_owned_action_context(original)?;
    let inquiry_id = action_payload(original, base_action)?;
    validate_identifier(inquiry_id)?;
    let mut canary = read_active_canary(inquiry_id)?;
    let mut withdrawals = Vec::new();
    for family in canary.families.iter().rev() {
        if let Some(receipt) =
            self_control_v2::withdraw_active_receipt(conv, &family.lease_receipt_id, original)?
        {
            withdrawals.push(receipt.receipt_id);
        }
    }
    let now = volition::now_unix_ms();
    let withdrawal = observation(
        inquiry_id,
        InquiryObservationPhaseV1::Withdrawal,
        now,
        withdrawals.clone(),
        json!({
            "source_intent_id": context.intent_id,
            "withdrawal_receipt_ids": withdrawals,
            "exact_named_canary": canary.canary_id,
            "felt_effect_established": false,
        }),
        InquiryMachineStatusV1::RolledBack,
    );
    let post = machine_status_observation(
        inquiry_id,
        InquiryObservationPhaseV1::PostRollback,
        now,
        &canary,
        InquiryMachineStatusV1::RolledBack,
    )?;
    canary.status = OwnerInquiryStatusV1::RolledBack;
    canary.rollback_state = InquiryRollbackStateV1::Withdrawn;
    canary
        .observations
        .extend([withdrawal.clone(), post.clone()]);
    canary.updated_at_unix_ms = now;
    write_canary(&canary)?;
    update_receipt_after_canary(
        inquiry_id,
        &[withdrawal, post],
        InquiryRollbackStateV1::Withdrawn,
        InquiryMachineStatusV1::RolledBack,
    )?;
    clear_active_canary(
        inquiry_id,
        OwnerInquiryStatusV1::RolledBack,
        &canary.canary_id,
    )?;
    research::mark_canary_terminal_if_present(
        inquiry_id,
        astrid_minime_protocol::OwnerResearchLifecycleStatusV1::RolledBack,
        InquiryMachineStatusV1::RolledBack,
        now,
    )?;
    Ok(format!(
        "Canary `{}` was withdrawn by exact intent `{}`. {} signed withdrawal receipts were returned; felt status remains unreported unless Astrid chooses to report it.",
        canary.canary_id,
        context.intent_id,
        withdrawals.len()
    ))
}

pub(super) fn promote_canary(
    conv: &mut ConversationState,
    original: &str,
    base_action: &str,
) -> Result<String, String> {
    let context = volition::exact_self_owned_action_context(original)?;
    let inquiry_id = action_payload(original, base_action)?;
    validate_identifier(inquiry_id)?;
    let mut canary = read_active_canary(inquiry_id)?;
    let mut withdrawal_receipts = Vec::new();
    for family in canary.families.iter().rev() {
        if let Some(receipt) =
            self_control_v2::withdraw_active_receipt(conv, &family.lease_receipt_id, original)?
        {
            withdrawal_receipts.push(receipt.receipt_id);
        }
    }
    let mut standing = Vec::<SelfControlReceiptV2>::new();
    for family in &canary.families {
        match self_control_v2::issue_standing(conv, family.family, family.values.clone(), original)
        {
            Ok(receipt)
                if matches!(
                    receipt.status,
                    SelfControlReceiptStatusV2::Applied | SelfControlReceiptStatusV2::Duplicate
                ) && receipt.requested_values == family.values
                    && receipt.clamped_values == family.values
                    && receipt.applied_values == family.values =>
            {
                standing.push(receipt);
            },
            Ok(receipt) => {
                if matches!(
                    receipt.status,
                    SelfControlReceiptStatusV2::Applied | SelfControlReceiptStatusV2::Duplicate
                ) {
                    standing.push(receipt.clone());
                }
                let rollback = rollback_standing(conv, &standing, original);
                let record = record_promotion_failure(
                    &mut canary,
                    "standing receipt substituted the exact owner-selected values",
                    &rollback,
                    standing
                        .iter()
                        .map(|receipt| receipt.receipt_id.clone())
                        .collect(),
                );
                return Err(format!(
                    "promotion receipt {} substituted an exact value; standing changes rollback={rollback}; evidence={record}",
                    receipt.receipt_id
                ));
            },
            Err(error) => {
                let rollback = rollback_standing(conv, &standing, original);
                let record = record_promotion_failure(
                    &mut canary,
                    &format!("standing issue failed: {error}"),
                    &rollback,
                    standing
                        .iter()
                        .map(|receipt| receipt.receipt_id.clone())
                        .collect(),
                );
                return Err(format!(
                    "promotion failed after lease withdrawal: {error}; standing changes rollback={rollback}; evidence={record}"
                ));
            },
        }
    }
    for (family, receipt) in canary.families.iter_mut().zip(&standing) {
        family.standing_intent_id = Some(receipt.intent_id.clone());
        family.standing_receipt_id = Some(receipt.receipt_id.clone());
    }
    let now = volition::now_unix_ms();
    let promotion_ids = standing
        .iter()
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let promotion = observation(
        inquiry_id,
        InquiryObservationPhaseV1::Promotion,
        now,
        promotion_ids.clone(),
        json!({
            "source_intent_id": context.intent_id,
            "lease_withdrawal_receipt_ids": withdrawal_receipts,
            "standing_receipt_ids": promotion_ids,
            "owner_explicitly_promoted": true,
            "silence_used_as_assent": false,
            "felt_effect_established": false,
        }),
        InquiryMachineStatusV1::Promoted,
    );
    canary.status = OwnerInquiryStatusV1::Promoted;
    canary.rollback_state = InquiryRollbackStateV1::Promoted;
    canary.observations.push(promotion.clone());
    canary.updated_at_unix_ms = now;
    let persistence = (|| {
        write_canary(&canary)?;
        update_receipt_after_canary(
            inquiry_id,
            &[promotion],
            InquiryRollbackStateV1::Promoted,
            InquiryMachineStatusV1::Promoted,
        )?;
        clear_active_canary(
            inquiry_id,
            OwnerInquiryStatusV1::Promoted,
            &canary.canary_id,
        )?;
        research::mark_canary_terminal_if_present(
            inquiry_id,
            astrid_minime_protocol::OwnerResearchLifecycleStatusV1::Promoted,
            InquiryMachineStatusV1::Promoted,
            now,
        )
    })();
    if let Err(error) = persistence {
        let rollback = rollback_standing(conv, &standing, original);
        let record = record_promotion_failure(
            &mut canary,
            &format!("promotion evidence persistence failed: {error}"),
            &rollback,
            promotion_ids,
        );
        return Err(format!(
            "promotion evidence failed after standing control issue; standing changes rollback={rollback}; evidence={record}"
        ));
    }
    Ok(format!(
        "Canary `{}` was explicitly promoted by exact intent `{}` into {} signed standing Self-Control V2 preferences. This establishes machine state only, not felt success or continued consent.",
        canary.canary_id,
        context.intent_id,
        standing.len()
    ))
}

pub(super) fn reconcile_canaries() -> Result<(), String> {
    let _guard = record_guard()?;
    let canary_dir = inquiry_root().join("canaries");
    let Ok(entries) = fs::read_dir(&canary_dir) else {
        return Ok(());
    };
    for entry in entries.filter_map(Result::ok) {
        let Some(mut canary) = volition::read_json::<InquiryCanaryV1>(&entry.path())? else {
            continue;
        };
        if canary.schema != INQUIRY_CANARY_SCHEMA_V1
            || canary.status != OwnerInquiryStatusV1::CanaryActive
        {
            continue;
        }
        let now = volition::now_unix_ms();
        let mut additions = Vec::new();
        if now >= canary.sample_one_due_at_unix_ms
            && !has_phase(&canary, InquiryObservationPhaseV1::DuringSampleOne)
        {
            additions.push(machine_status_observation(
                &canary.inquiry_id,
                InquiryObservationPhaseV1::DuringSampleOne,
                now,
                &canary,
                InquiryMachineStatusV1::Established,
            )?);
        }
        if now >= canary.sample_two_due_at_unix_ms
            && !has_phase(&canary, InquiryObservationPhaseV1::DuringSampleTwo)
        {
            additions.push(machine_status_observation(
                &canary.inquiry_id,
                InquiryObservationPhaseV1::DuringSampleTwo,
                now,
                &canary,
                InquiryMachineStatusV1::Established,
            )?);
        }
        let status = self_control_v2::status_at_root(&self_control_root())?;
        let active_intents = status
            .active_controls
            .iter()
            .map(|active| active.intent_id.as_str())
            .collect::<HashSet<_>>();
        let all_inactive = canary
            .families
            .iter()
            .all(|family| !active_intents.contains(family.lease_intent_id.as_str()));
        if all_inactive {
            if now >= canary.expires_at_unix_ms
                && !has_phase(&canary, InquiryObservationPhaseV1::Expiry)
            {
                additions.push(observation(
                    &canary.inquiry_id,
                    InquiryObservationPhaseV1::Expiry,
                    now,
                    Vec::new(),
                    json!({
                        "scheduled_expiry_unix_ms": canary.expires_at_unix_ms,
                        "all_lease_intents_inactive": true,
                        "silence_used_as_assent": false,
                        "felt_effect_established": false,
                    }),
                    InquiryMachineStatusV1::RolledBack,
                ));
            }
            if !has_phase(&canary, InquiryObservationPhaseV1::PostRollback) {
                additions.push(machine_status_observation(
                    &canary.inquiry_id,
                    InquiryObservationPhaseV1::PostRollback,
                    now,
                    &canary,
                    InquiryMachineStatusV1::RolledBack,
                )?);
            }
            canary.status = OwnerInquiryStatusV1::RolledBack;
            canary.rollback_state = InquiryRollbackStateV1::RolledBack;
        }
        if !additions.is_empty() || canary.status == OwnerInquiryStatusV1::RolledBack {
            canary.observations.extend(additions.clone());
            canary.updated_at_unix_ms = now;
            write_canary_unlocked(&canary)?;
            update_receipt_after_canary_unlocked(
                &canary.inquiry_id,
                &additions,
                canary.rollback_state,
                if canary.status == OwnerInquiryStatusV1::RolledBack {
                    InquiryMachineStatusV1::RolledBack
                } else {
                    InquiryMachineStatusV1::Established
                },
            )?;
            if canary.status == OwnerInquiryStatusV1::RolledBack {
                clear_active_canary_unlocked(
                    &canary.inquiry_id,
                    OwnerInquiryStatusV1::RolledBack,
                    &canary.canary_id,
                )?;
                research::mark_canary_terminal_if_present(
                    &canary.inquiry_id,
                    astrid_minime_protocol::OwnerResearchLifecycleStatusV1::RolledBack,
                    InquiryMachineStatusV1::RolledBack,
                    now,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_canary_values(values: &AstridCanaryValuesV1) -> Result<(), String> {
    let count = [
        values.semantic_strand_retention_turns.is_some(),
        values.conversation_temperature.is_some(),
        values.response_token_limit.is_some(),
        values.aperture.is_some(),
        values.continuity_readout.is_some(),
        values.vibrancy_aperture.is_some(),
        values.semantic_emission_gain.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if count == 0 {
        return Err("INQUIRY_CANARY requires at least one exact local value".to_string());
    }
    validate_u32(
        "semantic_strand_retention_turns",
        values.semantic_strand_retention_turns,
        0,
        32,
    )?;
    validate_f32(
        "conversation_temperature",
        values.conversation_temperature,
        0.1,
        1.5,
    )?;
    validate_u32(
        "response_token_limit",
        values.response_token_limit,
        128,
        1_536,
    )?;
    validate_f32("aperture", values.aperture, 0.0, 1.0)?;
    validate_f32("continuity_readout", values.continuity_readout, 0.0, 1.0)?;
    validate_f32("vibrancy_aperture", values.vibrancy_aperture, 0.0, 1.0)?;
    validate_f32(
        "semantic_emission_gain",
        values.semantic_emission_gain,
        0.5,
        5.0,
    )
}

fn validate_f32(name: &str, value: Option<f32>, low: f32, high: f32) -> Result<(), String> {
    if value.is_some_and(|value| !value.is_finite() || !(low..=high).contains(&value)) {
        return Err(format!(
            "{name} must be finite and within {low}..={high}; the value was rejected rather than clamped"
        ));
    }
    Ok(())
}

fn validate_u32(name: &str, value: Option<u32>, low: u32, high: u32) -> Result<(), String> {
    if value.is_some_and(|value| !(low..=high).contains(&value)) {
        return Err(format!(
            "{name} must be within {low}..={high}; the value was rejected rather than clamped"
        ));
    }
    Ok(())
}

fn canary_value_groups(
    values: &AstridCanaryValuesV1,
) -> Vec<(SelfControlFamilyV2, SelfControlValuesV2)> {
    let mut groups = Vec::new();
    if values.conversation_temperature.is_some()
        || values.response_token_limit.is_some()
        || values.aperture.is_some()
        || values.continuity_readout.is_some()
    {
        groups.push((
            SelfControlFamilyV2::Conversation,
            SelfControlValuesV2 {
                conversation_temperature: values.conversation_temperature,
                response_token_limit: values.response_token_limit,
                aperture: values.aperture,
                continuity_readout: values.continuity_readout,
                ..SelfControlValuesV2::default()
            },
        ));
    }
    if values.semantic_strand_retention_turns.is_some() {
        groups.push((
            SelfControlFamilyV2::SemanticContinuity,
            SelfControlValuesV2 {
                semantic_strand_retention_turns: values.semantic_strand_retention_turns,
                ..SelfControlValuesV2::default()
            },
        ));
    }
    if values.vibrancy_aperture.is_some() || values.semantic_emission_gain.is_some() {
        groups.push((
            SelfControlFamilyV2::SemanticEmission,
            SelfControlValuesV2 {
                vibrancy_aperture: values.vibrancy_aperture,
                semantic_emission_gain: values.semantic_emission_gain,
                ..SelfControlValuesV2::default()
            },
        ));
    }
    groups
}

fn rollback_issued(
    conv: &mut ConversationState,
    issued: &[(
        SelfControlFamilyV2,
        SelfControlValuesV2,
        SelfControlReceiptV2,
    )],
    source_action: &str,
) -> String {
    let mut results = Vec::new();
    for (_, _, receipt) in issued.iter().rev() {
        results.push(
            self_control_v2::withdraw_active_receipt(conv, &receipt.receipt_id, source_action)
                .map(|withdrawal| {
                    withdrawal.map_or_else(
                        || format!("{} already inactive", receipt.receipt_id),
                        |withdrawal| format!("{}->{}", receipt.receipt_id, withdrawal.receipt_id),
                    )
                })
                .unwrap_or_else(|error| format!("{} failed:{error}", receipt.receipt_id)),
        );
    }
    results.join(",")
}

fn rollback_standing(
    conv: &mut ConversationState,
    receipts: &[SelfControlReceiptV2],
    source_action: &str,
) -> String {
    let issued = receipts
        .iter()
        .map(|receipt| {
            (
                SelfControlFamilyV2::Conversation,
                receipt.requested_values.clone(),
                receipt.clone(),
            )
        })
        .collect::<Vec<_>>();
    rollback_issued(conv, &issued, source_action)
}

fn record_promotion_failure(
    canary: &mut InquiryCanaryV1,
    reason: &str,
    rollback: &str,
    standing_receipt_ids: Vec<String>,
) -> String {
    let now = volition::now_unix_ms();
    let rollback_failed = rollback.contains(" failed:");
    let machine_status = if rollback_failed {
        InquiryMachineStatusV1::Failed
    } else {
        InquiryMachineStatusV1::RolledBack
    };
    let rollback_state = if rollback_failed {
        InquiryRollbackStateV1::Failed
    } else {
        InquiryRollbackStateV1::RolledBack
    };
    let status = if rollback_failed {
        OwnerInquiryStatusV1::Failed
    } else {
        OwnerInquiryStatusV1::RolledBack
    };
    let failure = observation(
        &canary.inquiry_id,
        InquiryObservationPhaseV1::PostRollback,
        now,
        standing_receipt_ids,
        json!({
            "promotion_failure": reason,
            "standing_rollback": rollback,
            "machine_status_only": true,
            "felt_effect_established": false,
        }),
        machine_status,
    );
    canary.status = status;
    canary.rollback_state = rollback_state;
    canary.observations.push(failure.clone());
    canary.updated_at_unix_ms = now;
    let mut evidence = Vec::new();
    if let Err(error) = write_canary(canary) {
        evidence.push(format!("canary:{error}"));
    }
    if let Err(error) = update_receipt_after_canary(
        &canary.inquiry_id,
        &[failure],
        rollback_state,
        machine_status,
    ) {
        evidence.push(format!("receipt:{error}"));
    }
    if let Err(error) = clear_active_canary(&canary.inquiry_id, status, &canary.canary_id) {
        evidence.push(format!("runtime:{error}"));
    }
    let lifecycle_status = if rollback_failed {
        astrid_minime_protocol::OwnerResearchLifecycleStatusV1::Failed
    } else {
        astrid_minime_protocol::OwnerResearchLifecycleStatusV1::RolledBack
    };
    if let Err(error) = research::mark_canary_terminal_if_present(
        &canary.inquiry_id,
        lifecycle_status,
        machine_status,
        now,
    ) {
        evidence.push(format!("research:{error}"));
    }
    if evidence.is_empty() {
        "recorded".to_string()
    } else {
        format!("best-effort record errors={}", evidence.join(","))
    }
}

fn machine_status_observation(
    inquiry_id: &str,
    phase: InquiryObservationPhaseV1,
    now: u64,
    canary: &InquiryCanaryV1,
    machine_status: InquiryMachineStatusV1,
) -> Result<InquiryObservationV1, String> {
    let status = self_control_v2::status_at_root(&self_control_root())?;
    let evidence = serde_json::to_value(&status)
        .map_err(|error| format!("encode Self-Control V2 status evidence: {error}"))?;
    Ok(observation(
        inquiry_id,
        phase,
        now,
        canary
            .families
            .iter()
            .map(|family| family.lease_receipt_id.clone())
            .collect(),
        json!({
            "self_control_v2_status": evidence,
            "named_canary_id": canary.canary_id,
            "machine_status_only": true,
            "felt_effect_established": false,
        }),
        machine_status,
    ))
}

fn observation(
    inquiry_id: &str,
    phase: InquiryObservationPhaseV1,
    now: u64,
    self_control_receipt_ids: Vec<String>,
    machine_evidence: Value,
    machine_status: InquiryMachineStatusV1,
) -> InquiryObservationV1 {
    let machine_evidence_sha256 = exact_value_sha256(&machine_evidence);
    InquiryObservationV1 {
        schema: INQUIRY_OBSERVATION_SCHEMA_V1.to_string(),
        observation_id: format!(
            "{}-{}-{}",
            inquiry_id,
            phase_name(phase),
            short_hash(&machine_evidence_sha256)
        ),
        inquiry_id: inquiry_id.to_string(),
        phase,
        observed_at_unix_ms: now,
        self_control_receipt_ids,
        machine_evidence,
        machine_evidence_sha256,
        machine_status,
        felt_status: InquiryFeltStatusV1::Unreported,
        felt_report_ref: None,
    }
}

fn update_receipt_after_canary(
    inquiry_id: &str,
    additions: &[InquiryObservationV1],
    rollback_state: InquiryRollbackStateV1,
    machine_status: InquiryMachineStatusV1,
) -> Result<(), String> {
    let _guard = record_guard()?;
    update_receipt_after_canary_unlocked(inquiry_id, additions, rollback_state, machine_status)
}

fn update_receipt_after_canary_unlocked(
    inquiry_id: &str,
    additions: &[InquiryObservationV1],
    rollback_state: InquiryRollbackStateV1,
    machine_status: InquiryMachineStatusV1,
) -> Result<(), String> {
    let mut receipt = read_receipt(inquiry_id)?;
    for addition in additions {
        if !receipt
            .observations
            .iter()
            .any(|existing| existing.observation_id == addition.observation_id)
        {
            receipt.observations.push(addition.clone());
        }
    }
    receipt.rollback_state = rollback_state;
    receipt.machine_status = machine_status;
    volition::write_owner_json(&receipt_path(inquiry_id), &receipt)
}

fn clear_active_canary(
    inquiry_id: &str,
    status: OwnerInquiryStatusV1,
    canary_id: &str,
) -> Result<(), String> {
    let _guard = record_guard()?;
    clear_active_canary_unlocked(inquiry_id, status, canary_id)
}

fn clear_active_canary_unlocked(
    inquiry_id: &str,
    status: OwnerInquiryStatusV1,
    canary_id: &str,
) -> Result<(), String> {
    let mut runtime = read_runtime(inquiry_id)?;
    if runtime.active_canary_id.as_deref() == Some(canary_id) {
        runtime.active_canary_id = None;
    }
    runtime.status = status;
    runtime.updated_at_unix_ms = volition::now_unix_ms();
    volition::write_owner_json(&runtime_path(inquiry_id), &runtime)
}

fn has_phase(canary: &InquiryCanaryV1, phase: InquiryObservationPhaseV1) -> bool {
    canary
        .observations
        .iter()
        .any(|observation| observation.phase == phase)
}

fn read_active_canary(inquiry_id: &str) -> Result<InquiryCanaryV1, String> {
    let runtime = read_runtime(inquiry_id)?;
    let canary_id = runtime
        .active_canary_id
        .ok_or_else(|| format!("inquiry `{inquiry_id}` has no active canary"))?;
    let canary = volition::read_json::<InquiryCanaryV1>(&canary_path(&canary_id))?
        .ok_or_else(|| format!("active canary `{canary_id}` evidence is missing"))?;
    if canary.status != OwnerInquiryStatusV1::CanaryActive {
        return Err(format!("canary `{canary_id}` is not active"));
    }
    Ok(canary)
}

fn write_canary(canary: &InquiryCanaryV1) -> Result<(), String> {
    let _guard = record_guard()?;
    write_canary_unlocked(canary)
}

fn write_canary_unlocked(canary: &InquiryCanaryV1) -> Result<(), String> {
    volition::write_owner_json(&canary_path(&canary.canary_id), canary)
}

const fn default_canary_duration() -> u64 {
    DEFAULT_CANARY_DURATION_SECS
}

const fn phase_name(phase: InquiryObservationPhaseV1) -> &'static str {
    match phase {
        InquiryObservationPhaseV1::Baseline => "baseline",
        InquiryObservationPhaseV1::DuringSampleOne => "during-one",
        InquiryObservationPhaseV1::DuringSampleTwo => "during-two",
        InquiryObservationPhaseV1::Expiry => "expiry",
        InquiryObservationPhaseV1::Withdrawal => "withdrawal",
        InquiryObservationPhaseV1::PostRollback => "post-rollback",
        InquiryObservationPhaseV1::Promotion => "promotion",
    }
}

const fn canary_family_name(family: SelfControlFamilyV2) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_canary_ranges_reject_instead_of_clamp() {
        let values = AstridCanaryValuesV1 {
            conversation_temperature: Some(1.51),
            ..AstridCanaryValuesV1::default()
        };
        assert!(
            validate_canary_values(&values)
                .expect_err("outside range")
                .contains("rejected rather than clamped")
        );
    }

    #[test]
    fn value_groups_keep_families_separate() {
        let values = AstridCanaryValuesV1 {
            semantic_strand_retention_turns: Some(5),
            aperture: Some(0.7),
            semantic_emission_gain: Some(1.2),
            ..AstridCanaryValuesV1::default()
        };
        let groups = canary_value_groups(&values);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, SelfControlFamilyV2::Conversation);
        assert_eq!(groups[1].0, SelfControlFamilyV2::SemanticContinuity);
        assert_eq!(groups[2].0, SelfControlFamilyV2::SemanticEmission);
    }
}
