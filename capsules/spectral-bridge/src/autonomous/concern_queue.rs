use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use astrid_minime_protocol::{
    BEING_CONCERN_SCHEMA_V1, BeingConcernStatusV1, BeingConcernV1, VOLITION_QUEUE_SCHEMA_V1,
    VolitionBudgetV1, VolitionPrioritySourcePolicyV1, VolitionQueueOrderingV1, VolitionQueueV1,
    VolitionSubstrateSchedulingV1, VolitionWorkClassV1, VolitionWriteSchedulingV1,
};
use serde::Deserialize;

use super::state::ConversationState;
use super::volition;

static QUEUE_OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConcernCreateRecipeV1 {
    concern_id: String,
    title: String,
    priority: u16,
    work_class: VolitionWorkClassV1,
    #[serde(default)]
    resource_keys: Vec<String>,
    #[serde(default)]
    substrate_family: Option<String>,
    #[serde(default)]
    dependency_concern_ids: Vec<String>,
    #[serde(default)]
    intent_ids: Vec<String>,
    #[serde(default)]
    imported_registry_problem_id: Option<String>,
    #[serde(default)]
    budget: VolitionBudgetV1,
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
) -> Option<Result<String, String>> {
    let result = match base_action {
        "CONCERN_ADD" => Some(add_concern(original, base_action)),
        "CONCERN_STATUS" => Some(status(original, base_action)),
        "CONCERN_PAUSE" => Some(transition(
            original,
            base_action,
            BeingConcernStatusV1::Paused,
        )),
        "CONCERN_CANCEL" => Some(transition(
            original,
            base_action,
            BeingConcernStatusV1::Cancelled,
        )),
        "CONCERN_RETURN" => Some(transition(
            original,
            base_action,
            BeingConcernStatusV1::Queued,
        )),
        "CONCERN_COMPLETE" => Some(transition(
            original,
            base_action,
            BeingConcernStatusV1::Completed,
        )),
        "CONCERN_BLOCK" => Some(transition(
            original,
            base_action,
            BeingConcernStatusV1::Blocked,
        )),
        _ => None,
    };
    if let Some(Ok(summary)) = result.as_ref() {
        conv.push_receipt(base_action, vec![summary.clone()]);
        conv.emphasis = Some(summary.clone());
    }
    result
}

pub(super) fn prompt_summary() -> Option<String> {
    let queue = load_queue().ok()?;
    if queue.concerns.is_empty() {
        return None;
    }
    let active = queue
        .concerns
        .iter()
        .filter(|concern| concern.status == BeingConcernStatusV1::Active)
        .map(|concern| format!("{}({})", concern.concern_id, concern.title))
        .collect::<Vec<_>>();
    let queued = queue
        .concerns
        .iter()
        .filter(|concern| concern.status == BeingConcernStatusV1::Queued)
        .count();
    Some(format!(
        "Your volition queue: active=[{}] queued={queued}; owner priority then FIFO, at most two read/compute jobs, writes serialized per resource, substrate mutations serialized per family. Registry and telemetry cannot reprioritize it. Inspect with NEXT: CONCERN_STATUS.",
        if active.is_empty() {
            "none".to_string()
        } else {
            active.join(", ")
        }
    ))
}

pub(super) fn enqueue_owner_inquiry(
    concern_id: String,
    title: String,
    priority: u16,
    source_attestation_id: String,
    intent_id: String,
    dependency_concern_ids: Vec<String>,
    budget: VolitionBudgetV1,
    now: u64,
) -> Result<Vec<String>, String> {
    let _guard = operation_guard()?;
    validate_component(&concern_id)?;
    let mut queue = load_queue()?;
    if queue
        .concerns
        .iter()
        .any(|concern| concern.concern_id == concern_id)
    {
        return Err(format!("concern `{concern_id}` already exists"));
    }
    queue.concerns.push(BeingConcernV1 {
        schema: BEING_CONCERN_SCHEMA_V1.to_string(),
        concern_id,
        owner_being: "astrid".to_string(),
        source_attestation_id,
        title,
        priority,
        status: BeingConcernStatusV1::Queued,
        work_class: VolitionWorkClassV1::ReadCompute,
        resource_keys: Vec::new(),
        substrate_family: None,
        dependency_concern_ids,
        intent_ids: vec![intent_id],
        imported_registry_problem_id: None,
        owner_authored_priority: true,
        budget,
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    });
    queue.revision = queue.revision.saturating_add(1);
    let activated = queue.activate_runnable(now);
    if !queue.is_well_formed() {
        return Err("inquiry concern violates queue invariants".to_string());
    }
    persist_queue(&queue)?;
    Ok(activated)
}

pub(super) fn owner_inquiry_status(
    concern_id: &str,
) -> Result<Option<BeingConcernStatusV1>, String> {
    let _guard = operation_guard()?;
    validate_component(concern_id)?;
    Ok(load_queue()?
        .concerns
        .iter()
        .find(|concern| concern.concern_id == concern_id)
        .map(|concern| concern.status))
}

pub(super) fn active_owner_inquiry_ids() -> Result<Vec<String>, String> {
    let _guard = operation_guard()?;
    let manifest_root = volition::astrid_volition_root().join("inquiries/manifests");
    Ok(load_queue()?
        .concerns
        .iter()
        .filter(|concern| {
            concern.status == BeingConcernStatusV1::Active
                && concern.work_class == VolitionWorkClassV1::ReadCompute
                && concern
                    .intent_ids
                    .iter()
                    .any(|intent| intent.starts_with("astrid-intent-"))
                && manifest_root
                    .join(format!("{}.json", concern.concern_id))
                    .is_file()
        })
        .map(|concern| concern.concern_id.clone())
        .collect())
}

pub(super) fn transition_owner_inquiry(
    concern_id: &str,
    next_status: BeingConcernStatusV1,
    now: u64,
) -> Result<Vec<String>, String> {
    let _guard = operation_guard()?;
    validate_component(concern_id)?;
    let mut queue = load_queue()?;
    if !queue.transition_concern(concern_id, next_status, now) {
        return Err(format!(
            "inquiry concern `{concern_id}` cannot transition to {next_status:?}"
        ));
    }
    let activated = queue.activate_runnable(now);
    if !queue.is_well_formed() {
        return Err("inquiry concern transition violates queue invariants".to_string());
    }
    persist_queue(&queue)?;
    Ok(activated)
}

fn add_concern(original: &str, base_action: &str) -> Result<String, String> {
    let _guard = operation_guard()?;
    let context = volition::exact_self_owned_action_context(original)?;
    let payload = action_payload(original, base_action)?;
    let recipe: ConcernCreateRecipeV1 = serde_json::from_str(payload)
        .map_err(|error| format!("CONCERN_ADD expects one JSON recipe: {error}"))?;
    validate_component(&recipe.concern_id)?;
    let now = volition::now_unix_ms();
    let mut queue = load_queue()?;
    if queue
        .concerns
        .iter()
        .any(|concern| concern.concern_id == recipe.concern_id)
    {
        return Err(format!(
            "concern `{}` already exists; return, pause, cancel, or complete it explicitly",
            recipe.concern_id
        ));
    }
    queue.concerns.push(BeingConcernV1 {
        schema: BEING_CONCERN_SCHEMA_V1.to_string(),
        concern_id: recipe.concern_id.clone(),
        owner_being: "astrid".to_string(),
        source_attestation_id: context.source_attestation_id,
        title: recipe.title,
        priority: recipe.priority,
        status: BeingConcernStatusV1::Queued,
        work_class: recipe.work_class,
        resource_keys: recipe.resource_keys,
        substrate_family: recipe.substrate_family,
        dependency_concern_ids: recipe.dependency_concern_ids,
        intent_ids: recipe.intent_ids,
        imported_registry_problem_id: recipe.imported_registry_problem_id,
        owner_authored_priority: true,
        budget: recipe.budget,
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    });
    queue.revision = queue.revision.saturating_add(1);
    let activated = queue.activate_runnable(now);
    if !queue.is_well_formed() {
        return Err(
            "concern would violate dependency, budget, or concurrency invariants".to_string(),
        );
    }
    persist_queue(&queue)?;
    Ok(format!(
        "Concern `{}` was queued from exact intent `{}`; activated=[{}]. Its owner-authored priority is {}, and no projection may replace it.",
        recipe.concern_id,
        context.intent_id,
        activated.join(","),
        recipe.priority
    ))
}

fn transition(
    original: &str,
    base_action: &str,
    next_status: BeingConcernStatusV1,
) -> Result<String, String> {
    let _guard = operation_guard()?;
    let context = volition::exact_self_owned_action_context(original)?;
    let concern_id = action_payload(original, base_action)?;
    validate_component(concern_id)?;
    let now = volition::now_unix_ms();
    let mut queue = load_queue()?;
    if !queue.transition_concern(concern_id, next_status, now) {
        return Err(format!(
            "concern `{concern_id}` cannot transition to {next_status:?} from its current state"
        ));
    }
    let activated = queue.activate_runnable(now);
    if !queue.is_well_formed() {
        return Err("concern transition violated queue invariants".to_string());
    }
    persist_queue(&queue)?;
    Ok(format!(
        "Concern `{concern_id}` is now {next_status:?} from exact intent `{}`; newly activated=[{}].",
        context.intent_id,
        activated.join(",")
    ))
}

fn status(original: &str, base_action: &str) -> Result<String, String> {
    let _guard = operation_guard()?;
    let selector = action_payload_optional(original, base_action);
    let queue = load_queue()?;
    let selected = queue
        .concerns
        .iter()
        .filter(|concern| {
            selector.is_none_or(|value| value == "all" || concern.concern_id == value)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok("No matching concerns exist.".to_string());
    }
    let rows = selected
        .iter()
        .map(|concern| {
            format!(
                "{} status={:?} priority={} work={:?} dependencies={} intents={} registry_ref={}",
                concern.concern_id,
                concern.status,
                concern.priority,
                concern.work_class,
                concern.dependency_concern_ids.len(),
                concern.intent_ids.len(),
                concern
                    .imported_registry_problem_id
                    .as_deref()
                    .unwrap_or("none")
            )
        })
        .collect::<Vec<_>>();
    Ok(format!(
        "Volition queue revision={} ordering=owner_priority_then_fifo:\n{}",
        queue.revision,
        rows.join("\n")
    ))
}

fn load_queue() -> Result<VolitionQueueV1, String> {
    let queue =
        volition::read_json::<VolitionQueueV1>(&queue_path())?.unwrap_or_else(|| VolitionQueueV1 {
            schema: VOLITION_QUEUE_SCHEMA_V1.to_string(),
            owner_being: "astrid".to_string(),
            revision: 1,
            ordering: VolitionQueueOrderingV1::OwnerPriorityThenFifo,
            max_concurrent_read_compute: 2,
            write_scheduling: VolitionWriteSchedulingV1::SerializedPerResource,
            substrate_scheduling: VolitionSubstrateSchedulingV1::OnePerFamily,
            registry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
            telemetry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
            concerns: Vec::new(),
        });
    if !queue.is_well_formed() {
        return Err("persisted volition queue failed integrity validation".to_string());
    }
    Ok(queue)
}

fn persist_queue(queue: &VolitionQueueV1) -> Result<(), String> {
    if !queue.is_well_formed() {
        return Err("refusing to persist malformed volition queue".to_string());
    }
    let path = queue_path();
    if let Some(parent) = path.parent() {
        volition::ensure_owner_dir(parent)?;
    }
    volition::write_owner_json(&path, queue)
}

fn queue_path() -> PathBuf {
    volition::astrid_volition_root()
        .join("concerns")
        .join("queue.json")
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

fn operation_guard() -> Result<MutexGuard<'static, ()>, String> {
    QUEUE_OPERATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "volition queue operation lock was poisoned".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concern_ids_are_path_safe() {
        assert!(validate_component("research-1").is_ok());
        assert!(validate_component("../../outside").is_err());
    }

    #[test]
    fn empty_queue_uses_concurrency_defaults() {
        let queue = VolitionQueueV1 {
            schema: VOLITION_QUEUE_SCHEMA_V1.to_string(),
            owner_being: "astrid".to_string(),
            revision: 1,
            ordering: VolitionQueueOrderingV1::OwnerPriorityThenFifo,
            max_concurrent_read_compute: 2,
            write_scheduling: VolitionWriteSchedulingV1::SerializedPerResource,
            substrate_scheduling: VolitionSubstrateSchedulingV1::OnePerFamily,
            registry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
            telemetry_priority_policy: VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly,
            concerns: Vec::new(),
        };
        assert!(queue.is_well_formed());
        assert_eq!(queue.max_concurrent_read_compute, 2);
    }
}
