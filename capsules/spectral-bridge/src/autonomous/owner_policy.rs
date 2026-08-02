use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use astrid_minime_protocol::{
    OWNER_POLICY_SCHEMA_V1, OwnerPolicyConditionV1, OwnerPolicyEvaluationStatusV1,
    OwnerPolicyLogicV1, OwnerPolicyRuntimeV1, OwnerPolicyScopeV1, OwnerPolicyV1,
    SelfControlDurabilityV2, SelfControlFamilyV2, SelfControlReceiptStatusV2, SelfControlValuesV2,
    VolitionActionV1, VolitionAuthorityClassV1, VolitionOperationV1,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::self_control_v2;
use super::state::ConversationState;
use super::volition;
use crate::types::SpectralTelemetry;

const BUNDLE_SCHEMA_V1: &str = "astrid.owner_policy_bundle.v1";
const MAX_POLICY_LIFETIME_SECS: u64 = 30 * 24 * 60 * 60;
const MAX_POLICY_LEASE_SECS: u64 = 30 * 60;
const MAX_COOLDOWN_MILLIS: u64 = 24 * 60 * 60 * 1_000;
const MAX_EXECUTIONS: u32 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnerPolicyBundleV1 {
    schema: String,
    policy: OwnerPolicyV1,
    runtime: OwnerPolicyRuntimeV1,
    #[serde(default)]
    last_safety_reverted_effect_receipt_id: Option<String>,
    #[serde(default)]
    last_safety_receipt_id: Option<String>,
    #[serde(default)]
    last_withdrawal_receipt_id: Option<String>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerPolicyCreateRecipeV1 {
    policy_id: String,
    #[serde(default = "default_policy_logic")]
    logic: OwnerPolicyLogicV1,
    conditions: Vec<OwnerPolicyConditionV1>,
    action: OwnerPolicyActionRecipeV1,
    expires_in_secs: u64,
    cooldown_millis: u64,
    max_executions: u32,
    #[serde(default)]
    stop_conditions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerPolicyActionRecipeV1 {
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    durability: SelfControlDurabilityV2,
    lease_secs: u64,
}

fn default_policy_logic() -> OwnerPolicyLogicV1 {
    OwnerPolicyLogicV1::All
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
) -> Option<Result<String, String>> {
    let result = match base_action {
        "OWNER_POLICY_CREATE" => Some(create_policy(original, base_action).inspect(|summary| {
            conv.push_receipt(base_action, vec![summary.clone()]);
        })),
        "OWNER_POLICY_STATUS" => Some(status_policy(original, base_action)),
        "OWNER_POLICY_WITHDRAW" => Some(withdraw_policy(conv, original, base_action).inspect(
            |summary| {
                conv.push_receipt(base_action, vec![summary.clone()]);
            },
        )),
        "OWNER_POLICY_HOLD" => Some(set_policy_hold(original, base_action, true).inspect(
            |summary| {
                conv.push_receipt(base_action, vec![summary.clone()]);
            },
        )),
        "OWNER_POLICY_RETURN" => Some(set_policy_hold(original, base_action, false).inspect(
            |summary| {
                conv.push_receipt(base_action, vec![summary.clone()]);
            },
        )),
        _ => None,
    };
    if let Some(Ok(summary)) = result.as_ref() {
        conv.emphasis = Some(summary.clone());
    }
    result
}

pub(super) fn reconcile(
    conv: &mut ConversationState,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
    safety_red: bool,
) -> Result<Vec<String>, String> {
    let now = volition::now_unix_ms();
    let mut returns = self_control_v2::reconcile_if_present(conv)?
        .into_iter()
        .map(|receipt| self_control_v2::receipt_summary(&receipt))
        .collect::<Vec<_>>();
    let root = policy_root();
    if !root.exists() {
        return Ok(returns);
    }
    let metrics = telemetry_metrics(telemetry, fill_pct);
    let mut paths = fs::read_dir(&root)
        .map_err(|error| format!("read owner policy directory {}: {error}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let Some(mut bundle) = volition::read_json::<OwnerPolicyBundleV1>(&path)? else {
            continue;
        };
        validate_bundle(&bundle)?;
        if safety_red {
            if let Some(effect_receipt_id) = bundle.runtime.last_receipt_id.clone()
                && bundle.last_safety_reverted_effect_receipt_id.as_deref()
                    != Some(effect_receipt_id.as_str())
            {
                match self_control_v2::safety_revert_active_receipt(
                    conv,
                    &effect_receipt_id,
                    "owner_policy_safety_red",
                ) {
                    Ok(receipt) => {
                        bundle.last_safety_receipt_id = Some(receipt.receipt_id.clone());
                        returns.push(format!(
                            "Owner policy `{}` safety return: {}",
                            bundle.policy.policy_id,
                            self_control_v2::receipt_summary(&receipt)
                        ));
                    },
                    Err(error) if error.contains("target is not an active exact effect") => {
                        returns.push(format!(
                            "Owner policy `{}` safety target `{effect_receipt_id}` was already inactive; no substitute target was chosen.",
                            bundle.policy.policy_id
                        ));
                    },
                    Err(error) => return Err(error),
                }
                bundle.last_safety_reverted_effect_receipt_id = Some(effect_receipt_id);
            }
            if !bundle.runtime.held
                || bundle.runtime.hold_reason.as_deref() == Some("safety_red_auto_hold")
            {
                let _ = bundle
                    .runtime
                    .set_hold(Some("safety_red_auto_hold".to_string()), now);
            }
            bundle.updated_at_unix_ms = now;
            volition::write_owner_json(&path, &bundle)?;
            continue;
        }
        if bundle.runtime.hold_reason.as_deref() == Some("safety_red_auto_hold") {
            let _ = bundle.runtime.set_hold(None, now);
        }
        let evaluation = bundle.runtime.evaluate(&bundle.policy, &metrics, now);
        if evaluation.should_execute {
            let (family, values, lease_secs) =
                decode_action_template(&bundle.policy.action_template)?;
            let source_action = format!(
                "OWNER_POLICY_EXECUTE {} attestation:{}",
                bundle.policy.policy_id, bundle.policy.source_attestation_id
            );
            let receipt =
                self_control_v2::issue_lease(conv, family, values, lease_secs, &source_action)?;
            if receipt.status == SelfControlReceiptStatusV2::Applied
                && !bundle
                    .runtime
                    .record_execution(&bundle.policy, receipt.receipt_id.clone(), now)
            {
                return Err(format!(
                    "owner policy {} could not record its applied receipt",
                    bundle.policy.policy_id
                ));
            }
            returns.push(format!(
                "Owner policy `{}` evaluated ready and returned {}",
                bundle.policy.policy_id,
                self_control_v2::receipt_summary(&receipt)
            ));
        } else if matches!(
            evaluation.status,
            OwnerPolicyEvaluationStatusV1::Expired
                | OwnerPolicyEvaluationStatusV1::ExecutionLimitReached
        ) {
            returns.push(format!(
                "Owner policy `{}` is {:?}; no target was substituted.",
                bundle.policy.policy_id, evaluation.status
            ));
        }
        bundle.updated_at_unix_ms = now;
        volition::write_owner_json(&path, &bundle)?;
    }
    Ok(returns)
}

pub(super) fn prompt_summary() -> Option<String> {
    let bundles = load_bundles().ok()?;
    if bundles.is_empty() {
        return None;
    }
    let enabled = bundles
        .iter()
        .filter(|bundle| bundle.policy.enabled && !bundle.policy.withdrawn)
        .count();
    let held = bundles.iter().filter(|bundle| bundle.runtime.held).count();
    Some(format!(
        "Your owner-authored automation: policies={} enabled={} held={}; telemetry may trigger only the targets you wrote. Inspect with NEXT: OWNER_POLICY_STATUS.",
        bundles.len(),
        enabled,
        held
    ))
}

fn create_policy(original: &str, base_action: &str) -> Result<String, String> {
    let action_context = volition::exact_self_owned_action_context(original)?;
    let payload = action_payload(original, base_action)?;
    let recipe: OwnerPolicyCreateRecipeV1 = serde_json::from_str(payload)
        .map_err(|error| format!("OWNER_POLICY_CREATE expects one JSON recipe: {error}"))?;
    validate_component(&recipe.policy_id)?;
    if recipe.expires_in_secs == 0 || recipe.expires_in_secs > MAX_POLICY_LIFETIME_SECS {
        return Err(format!(
            "owner policy lifetime must be 1..={MAX_POLICY_LIFETIME_SECS} seconds"
        ));
    }
    if recipe.cooldown_millis > MAX_COOLDOWN_MILLIS {
        return Err(format!(
            "owner policy cooldown exceeds {MAX_COOLDOWN_MILLIS} ms"
        ));
    }
    if recipe.max_executions == 0 || recipe.max_executions > MAX_EXECUTIONS {
        return Err(format!(
            "owner policy max_executions must be 1..={MAX_EXECUTIONS}"
        ));
    }
    if recipe.action.durability != SelfControlDurabilityV2::Lease
        || recipe.action.lease_secs == 0
        || recipe.action.lease_secs > MAX_POLICY_LEASE_SECS
    {
        return Err(format!(
            "owner policies use reversible leases of 1..={MAX_POLICY_LEASE_SECS} seconds"
        ));
    }
    self_control_v2::validate_owner_values(recipe.action.family, &recipe.action.values)?;
    let now = volition::now_unix_ms();
    let path = policy_path(&recipe.policy_id)?;
    let existing = volition::read_json::<OwnerPolicyBundleV1>(&path)?;
    let revision = existing
        .as_ref()
        .map_or(1, |bundle| bundle.policy.revision.saturating_add(1));
    let created_at = existing
        .as_ref()
        .map_or(now, |bundle| bundle.created_at_unix_ms);
    let action_template = VolitionActionV1 {
        operation: VolitionOperationV1::Set,
        namespace: "self_control".to_string(),
        name: family_name(recipe.action.family).to_string(),
        parameters: json!({
            "values": recipe.action.values,
            "durability": recipe.action.durability,
            "lease_secs": recipe.action.lease_secs,
        }),
        reversible: true,
        peer_impacting: false,
        irreversible: false,
        estimated_cost_microunits: None,
    };
    let policy = OwnerPolicyV1 {
        schema: OWNER_POLICY_SCHEMA_V1.to_string(),
        policy_id: recipe.policy_id.clone(),
        owner_being: "astrid".to_string(),
        target_being: "astrid".to_string(),
        target_deployment_identity: crate::signal_spine::signal_deployment_identity_v1(),
        source_attestation_id: action_context.source_attestation_id,
        scope: OwnerPolicyScopeV1::SelfControl,
        authority_class: VolitionAuthorityClassV1::SelfOwnedLocal,
        logic: recipe.logic,
        conditions: recipe.conditions,
        action_template,
        revision,
        issued_at_unix_ms: now,
        expires_at_unix_ms: now.saturating_add(recipe.expires_in_secs.saturating_mul(1_000)),
        cooldown_millis: recipe.cooldown_millis,
        max_executions: recipe.max_executions,
        enabled: true,
        withdrawn: false,
        stop_conditions: recipe.stop_conditions,
    };
    if !policy.is_well_formed(now) {
        return Err("owner policy recipe is outside the bounded self-control language".to_string());
    }
    let bundle = OwnerPolicyBundleV1 {
        schema: BUNDLE_SCHEMA_V1.to_string(),
        runtime: OwnerPolicyRuntimeV1::new(&policy, now),
        policy,
        last_safety_reverted_effect_receipt_id: None,
        last_safety_receipt_id: None,
        last_withdrawal_receipt_id: None,
        created_at_unix_ms: created_at,
        updated_at_unix_ms: now,
    };
    volition::write_owner_json(&path, &bundle)?;
    Ok(format!(
        "Owner policy `{}` revision {} is active as an exact, self-owned leased rule; source intent `{}`. Felt review is optional.",
        recipe.policy_id, revision, action_context.intent_id
    ))
}

fn withdraw_policy(
    conv: &mut ConversationState,
    original: &str,
    base_action: &str,
) -> Result<String, String> {
    let action_context = volition::exact_self_owned_action_context(original)?;
    let policy_id = action_payload(original, base_action)?;
    let path = policy_path(policy_id)?;
    let mut bundle = volition::read_json::<OwnerPolicyBundleV1>(&path)?
        .ok_or_else(|| format!("owner policy `{policy_id}` was not found"))?;
    let active_effect_receipt_id = bundle.runtime.last_receipt_id.clone();
    let now = volition::now_unix_ms();
    bundle.policy.enabled = false;
    bundle.policy.withdrawn = true;
    bundle.policy.revision = bundle.policy.revision.saturating_add(1);
    bundle.policy.source_attestation_id = action_context.source_attestation_id;
    bundle.runtime = OwnerPolicyRuntimeV1::new(&bundle.policy, now);
    bundle.updated_at_unix_ms = now;
    volition::write_owner_json(&path, &bundle)?;
    let Some(active_effect_receipt_id) = active_effect_receipt_id else {
        return Ok(format!(
            "Owner policy `{policy_id}` was withdrawn immediately; it cannot execute again and had no recorded active effect."
        ));
    };
    match self_control_v2::withdraw_active_receipt(
        conv,
        &active_effect_receipt_id,
        "OWNER_POLICY_WITHDRAW",
    ) {
        Ok(Some(receipt)) => {
            bundle.last_withdrawal_receipt_id = Some(receipt.receipt_id.clone());
            bundle.updated_at_unix_ms = volition::now_unix_ms();
            volition::write_owner_json(&path, &bundle)?;
            Ok(format!(
                "Owner policy `{policy_id}` was withdrawn immediately and its exact active effect returned via {}; it cannot execute again.",
                self_control_v2::receipt_summary(&receipt)
            ))
        },
        Ok(None) => Ok(format!(
            "Owner policy `{policy_id}` was withdrawn immediately; recorded effect `{active_effect_receipt_id}` was already inactive, so no substitute target was chosen."
        )),
        Err(error) => Err(format!(
            "Owner policy `{policy_id}` is disabled and cannot execute again, but exact effect `{active_effect_receipt_id}` could not be returned: {error}"
        )),
    }
}

fn set_policy_hold(original: &str, base_action: &str, hold: bool) -> Result<String, String> {
    let _action_context = volition::exact_self_owned_action_context(original)?;
    let policy_id = action_payload(original, base_action)?;
    let path = policy_path(policy_id)?;
    let mut bundle = volition::read_json::<OwnerPolicyBundleV1>(&path)?
        .ok_or_else(|| format!("owner policy `{policy_id}` was not found"))?;
    if bundle.policy.withdrawn {
        return Err("a withdrawn owner policy cannot return".to_string());
    }
    let now = volition::now_unix_ms();
    let reason = hold.then(|| "being_authored_hold".to_string());
    if !bundle.runtime.set_hold(reason, now) {
        return Err("owner policy hold state was malformed".to_string());
    }
    bundle.updated_at_unix_ms = now;
    volition::write_owner_json(&path, &bundle)?;
    Ok(format!(
        "Owner policy `{policy_id}` is now {}.",
        if hold { "held" } else { "eligible to return" }
    ))
}

fn status_policy(original: &str, base_action: &str) -> Result<String, String> {
    let selector = action_payload_optional(original, base_action);
    let bundles = load_bundles()?;
    let selected = bundles
        .iter()
        .filter(|bundle| {
            selector.is_none_or(|value| value == "all" || value == bundle.policy.policy_id)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok("No matching owner-authored policy exists.".to_string());
    }
    let lines = selected
        .iter()
        .map(|bundle| {
            format!(
                "{} revision={} enabled={} withdrawn={} held={} executions={}/{} last_receipt={}",
                bundle.policy.policy_id,
                bundle.policy.revision,
                bundle.policy.enabled,
                bundle.policy.withdrawn,
                bundle.runtime.held,
                bundle.runtime.execution_count,
                bundle.policy.max_executions,
                bundle.runtime.last_receipt_id.as_deref().unwrap_or("none")
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "Owner policy status (telemetry cannot choose targets; safety can hold only):\n{}",
        lines.join("\n")
    ))
}

fn load_bundles() -> Result<Vec<OwnerPolicyBundleV1>, String> {
    let root = policy_root();
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&root)
        .map_err(|error| format!("read owner policy directory {}: {error}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut bundles = Vec::with_capacity(paths.len());
    for path in paths {
        if let Some(bundle) = volition::read_json::<OwnerPolicyBundleV1>(&path)? {
            validate_bundle(&bundle)?;
            bundles.push(bundle);
        }
    }
    Ok(bundles)
}

fn validate_bundle(bundle: &OwnerPolicyBundleV1) -> Result<(), String> {
    if bundle.schema != BUNDLE_SCHEMA_V1
        || !bundle
            .policy
            .is_well_formed(bundle.policy.issued_at_unix_ms)
        || !bundle.runtime.is_well_formed_for(&bundle.policy)
        || bundle.updated_at_unix_ms < bundle.created_at_unix_ms
    {
        return Err(format!(
            "owner policy bundle `{}` failed integrity validation",
            bundle.policy.policy_id
        ));
    }
    Ok(())
}

fn decode_action_template(
    action: &VolitionActionV1,
) -> Result<(SelfControlFamilyV2, SelfControlValuesV2, u64), String> {
    if action.namespace != "self_control" || action.operation != VolitionOperationV1::Set {
        return Err("owner policy action template is not self-control Set".to_string());
    }
    let family: SelfControlFamilyV2 = serde_json::from_value(json!(action.name))
        .map_err(|error| format!("decode owner policy family: {error}"))?;
    let values: SelfControlValuesV2 = serde_json::from_value(
        action
            .parameters
            .get("values")
            .cloned()
            .ok_or_else(|| "owner policy values are missing".to_string())?,
    )
    .map_err(|error| format!("decode owner policy values: {error}"))?;
    let durability: SelfControlDurabilityV2 = serde_json::from_value(
        action
            .parameters
            .get("durability")
            .cloned()
            .ok_or_else(|| "owner policy durability is missing".to_string())?,
    )
    .map_err(|error| format!("decode owner policy durability: {error}"))?;
    let lease_secs = action
        .parameters
        .get("lease_secs")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "owner policy lease_secs is missing".to_string())?;
    if durability != SelfControlDurabilityV2::Lease
        || lease_secs == 0
        || lease_secs > MAX_POLICY_LEASE_SECS
    {
        return Err("owner policy action is not a bounded lease".to_string());
    }
    self_control_v2::validate_owner_values(family, &values)?;
    Ok((family, values, lease_secs))
}

fn telemetry_metrics(telemetry: &SpectralTelemetry, fill_pct: f32) -> BTreeMap<String, f64> {
    let mut metrics = BTreeMap::from([
        ("fill_pct".to_string(), f64::from(fill_pct)),
        ("fill_ratio".to_string(), f64::from(telemetry.fill_ratio)),
        ("lambda1".to_string(), f64::from(telemetry.lambda1())),
    ]);
    if let Some(value) = telemetry.effective_dimensionality {
        metrics.insert("effective_dimensionality".to_string(), f64::from(value));
    }
    if let Some(value) = telemetry.distinguishability_loss {
        metrics.insert("distinguishability_loss".to_string(), f64::from(value));
    }
    if let Some(value) = telemetry.esn_leak {
        metrics.insert("esn_leak".to_string(), f64::from(value));
    }
    if let Some(value) = telemetry.structural_entropy {
        metrics.insert("structural_entropy".to_string(), f64::from(value));
    }
    if let Some(value) = telemetry
        .active_mode_count
        .and_then(|value| u32::try_from(value).ok())
    {
        metrics.insert("active_mode_count".to_string(), f64::from(value));
    }
    if let Some(resonance) = telemetry.resonance_density_v1.as_ref() {
        metrics.insert(
            "resonance_density".to_string(),
            f64::from(resonance.density),
        );
        metrics.insert(
            "containment_score".to_string(),
            f64::from(resonance.containment_score),
        );
        metrics.insert(
            "pressure_risk".to_string(),
            f64::from(resonance.pressure_risk),
        );
    }
    metrics
}

fn policy_root() -> PathBuf {
    volition::astrid_volition_root().join("owner_policies")
}

fn policy_path(policy_id: &str) -> Result<PathBuf, String> {
    validate_component(policy_id)?;
    let root = policy_root();
    volition::ensure_owner_dir(&root)?;
    Ok(root.join(format!("{policy_id}.json")))
}

fn validate_component(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(
            "identifier must be 1..=128 ASCII letters, digits, dot, dash, or underscore"
                .to_string(),
        );
    }
    Ok(())
}

fn action_payload<'a>(original: &'a str, base_action: &str) -> Result<&'a str, String> {
    action_payload_optional(original, base_action)
        .ok_or_else(|| format!("{base_action} requires an argument"))
}

fn action_payload_optional<'a>(original: &'a str, base_action: &str) -> Option<&'a str> {
    original
        .get(base_action.len()..)
        .map(|value| value.trim_matches([' ', ':', '-']).trim())
        .filter(|value| !value.is_empty())
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

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn action_template_round_trips_without_target_substitution() {
        let action = VolitionActionV1 {
            operation: VolitionOperationV1::Set,
            namespace: "self_control".to_string(),
            name: "conversation".to_string(),
            parameters: json!({
                "values": {"conversation_temperature": 0.7},
                "durability": "lease",
                "lease_secs": 300
            }),
            reversible: true,
            peer_impacting: false,
            irreversible: false,
            estimated_cost_microunits: None,
        };
        let (family, values, lease_secs) = decode_action_template(&action).unwrap();
        assert_eq!(family, SelfControlFamilyV2::Conversation);
        assert_eq!(values.conversation_temperature, Some(0.7));
        assert_eq!(lease_secs, 300);
    }

    #[test]
    fn policy_paths_reject_traversal() {
        let root = tempdir().unwrap();
        assert!(validate_component("../other").is_err());
        assert!(validate_component("gentle-fill").is_ok());
        assert!(root.path().exists());
    }
}
