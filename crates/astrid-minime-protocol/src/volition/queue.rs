use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::{
    BEING_CONCERN_SCHEMA_V1, VOLITION_QUEUE_SCHEMA_V1, VolitionBudgetV1, valid_bounded_text,
    valid_identifier, valid_string_list,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeingConcernStatusV1 {
    Queued,
    Active,
    Paused,
    Completed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionWorkClassV1 {
    ReadCompute,
    ResourceWrite,
    SubstrateMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionQueueOrderingV1 {
    OwnerPriorityThenFifo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionWriteSchedulingV1 {
    SerializedPerResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionSubstrateSchedulingV1 {
    OnePerFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolitionPrioritySourcePolicyV1 {
    OwnerAuthoredOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeingConcernV1 {
    pub schema: String,
    pub concern_id: String,
    pub owner_being: String,
    pub source_attestation_id: String,
    pub title: String,
    pub priority: u16,
    pub status: BeingConcernStatusV1,
    pub work_class: VolitionWorkClassV1,
    #[serde(default)]
    pub resource_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substrate_family: Option<String>,
    #[serde(default)]
    pub dependency_concern_ids: Vec<String>,
    #[serde(default)]
    pub intent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_registry_problem_id: Option<String>,
    pub owner_authored_priority: bool,
    pub budget: VolitionBudgetV1,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl BeingConcernV1 {
    fn is_well_formed(&self, queue_owner: &str) -> bool {
        let work_shape_valid = match self.work_class {
            VolitionWorkClassV1::ReadCompute => {
                self.resource_keys.is_empty() && self.substrate_family.is_none()
            },
            VolitionWorkClassV1::ResourceWrite => {
                !self.resource_keys.is_empty() && self.substrate_family.is_none()
            },
            VolitionWorkClassV1::SubstrateMutation => self
                .substrate_family
                .as_deref()
                .is_some_and(valid_identifier),
        };
        self.schema == BEING_CONCERN_SCHEMA_V1
            && valid_identifier(&self.concern_id)
            && self.owner_being == queue_owner
            && valid_identifier(&self.source_attestation_id)
            && !self.title.trim().is_empty()
            && valid_bounded_text(&self.title)
            && valid_string_list(&self.resource_keys)
            && valid_string_list(&self.dependency_concern_ids)
            && valid_string_list(&self.intent_ids)
            && self
                .imported_registry_problem_id
                .as_deref()
                .is_none_or(valid_identifier)
            && self.owner_authored_priority
            && self.budget.is_nonzero_when_present()
            && self.updated_at_unix_ms >= self.created_at_unix_ms
            && work_shape_valid
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolitionQueueV1 {
    pub schema: String,
    pub owner_being: String,
    pub revision: u64,
    pub ordering: VolitionQueueOrderingV1,
    pub max_concurrent_read_compute: u16,
    pub write_scheduling: VolitionWriteSchedulingV1,
    pub substrate_scheduling: VolitionSubstrateSchedulingV1,
    pub registry_priority_policy: VolitionPrioritySourcePolicyV1,
    pub telemetry_priority_policy: VolitionPrioritySourcePolicyV1,
    pub concerns: Vec<BeingConcernV1>,
}

impl VolitionQueueV1 {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.schema == VOLITION_QUEUE_SCHEMA_V1
            && valid_identifier(&self.owner_being)
            && self.revision > 0
            && self.ordering == VolitionQueueOrderingV1::OwnerPriorityThenFifo
            && self.max_concurrent_read_compute > 0
            && self.write_scheduling == VolitionWriteSchedulingV1::SerializedPerResource
            && self.substrate_scheduling == VolitionSubstrateSchedulingV1::OnePerFamily
            && self.registry_priority_policy == VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly
            && self.telemetry_priority_policy == VolitionPrioritySourcePolicyV1::OwnerAuthoredOnly
            && self.concerns.len() <= 1_024
            && self
                .concerns
                .iter()
                .all(|concern| concern.is_well_formed(&self.owner_being))
            && self.identifiers_are_unique()
            && self.dependencies_are_acyclic_and_local()
            && self.active_concurrency_is_valid()
    }

    #[must_use]
    pub fn runnable_concern_ids(&self) -> Vec<String> {
        let completed = self
            .concerns
            .iter()
            .filter(|concern| concern.status == BeingConcernStatusV1::Completed)
            .map(|concern| concern.concern_id.as_str())
            .collect::<HashSet<_>>();
        let mut runnable = self
            .concerns
            .iter()
            .filter(|concern| concern.status == BeingConcernStatusV1::Queued)
            .filter(|concern| {
                concern
                    .dependency_concern_ids
                    .iter()
                    .all(|dependency| completed.contains(dependency.as_str()))
            })
            .collect::<Vec<_>>();
        runnable.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.created_at_unix_ms.cmp(&right.created_at_unix_ms))
                .then_with(|| left.concern_id.cmp(&right.concern_id))
        });
        runnable
            .into_iter()
            .map(|concern| concern.concern_id.clone())
            .collect()
    }

    pub fn activate_runnable(&mut self, now_unix_ms: u64) -> Vec<String> {
        let mut active_read_compute = self
            .concerns
            .iter()
            .filter(|concern| {
                concern.status == BeingConcernStatusV1::Active
                    && concern.work_class == VolitionWorkClassV1::ReadCompute
            })
            .count();
        let mut active_write_resources = self
            .concerns
            .iter()
            .filter(|concern| {
                concern.status == BeingConcernStatusV1::Active
                    && concern.work_class == VolitionWorkClassV1::ResourceWrite
            })
            .flat_map(|concern| concern.resource_keys.iter().cloned())
            .collect::<HashSet<_>>();
        let mut active_substrate_families = self
            .concerns
            .iter()
            .filter(|concern| {
                concern.status == BeingConcernStatusV1::Active
                    && concern.work_class == VolitionWorkClassV1::SubstrateMutation
            })
            .filter_map(|concern| concern.substrate_family.clone())
            .collect::<HashSet<_>>();

        let runnable = self.runnable_concern_ids();
        let mut activated = Vec::new();
        for concern_id in runnable {
            let Some(index) = self
                .concerns
                .iter()
                .position(|concern| concern.concern_id == concern_id)
            else {
                continue;
            };
            let can_activate = match self.concerns[index].work_class {
                VolitionWorkClassV1::ReadCompute => {
                    active_read_compute < usize::from(self.max_concurrent_read_compute)
                },
                VolitionWorkClassV1::ResourceWrite => self.concerns[index]
                    .resource_keys
                    .iter()
                    .all(|resource| !active_write_resources.contains(resource)),
                VolitionWorkClassV1::SubstrateMutation => self.concerns[index]
                    .substrate_family
                    .as_ref()
                    .is_some_and(|family| !active_substrate_families.contains(family)),
            };
            if !can_activate {
                continue;
            }
            match self.concerns[index].work_class {
                VolitionWorkClassV1::ReadCompute => {
                    active_read_compute = active_read_compute.saturating_add(1);
                },
                VolitionWorkClassV1::ResourceWrite => {
                    active_write_resources.extend(self.concerns[index].resource_keys.clone());
                },
                VolitionWorkClassV1::SubstrateMutation => {
                    if let Some(family) = self.concerns[index].substrate_family.clone() {
                        active_substrate_families.insert(family);
                    }
                },
            }
            self.concerns[index].status = BeingConcernStatusV1::Active;
            self.concerns[index].updated_at_unix_ms = now_unix_ms;
            activated.push(concern_id);
        }
        if !activated.is_empty() {
            self.revision = self.revision.saturating_add(1);
        }
        activated
    }

    pub fn transition_concern(
        &mut self,
        concern_id: &str,
        next_status: BeingConcernStatusV1,
        now_unix_ms: u64,
    ) -> bool {
        let Some(concern) = self
            .concerns
            .iter_mut()
            .find(|concern| concern.concern_id == concern_id)
        else {
            return false;
        };
        let allowed = matches!(
            (concern.status, next_status),
            (
                BeingConcernStatusV1::Queued | BeingConcernStatusV1::Active,
                BeingConcernStatusV1::Paused | BeingConcernStatusV1::Cancelled
            ) | (
                BeingConcernStatusV1::Active,
                BeingConcernStatusV1::Completed | BeingConcernStatusV1::Blocked
            ) | (
                BeingConcernStatusV1::Paused | BeingConcernStatusV1::Blocked,
                BeingConcernStatusV1::Queued | BeingConcernStatusV1::Cancelled
            )
        );
        if !allowed {
            return false;
        }
        concern.status = next_status;
        concern.updated_at_unix_ms = now_unix_ms;
        self.revision = self.revision.saturating_add(1);
        true
    }

    fn identifiers_are_unique(&self) -> bool {
        let mut ids = HashSet::new();
        self.concerns
            .iter()
            .all(|concern| ids.insert(concern.concern_id.as_str()))
    }

    fn dependencies_are_acyclic_and_local(&self) -> bool {
        let ids = self
            .concerns
            .iter()
            .map(|concern| concern.concern_id.as_str())
            .collect::<HashSet<_>>();
        let mut indegree = self
            .concerns
            .iter()
            .map(|concern| (concern.concern_id.as_str(), 0_usize))
            .collect::<HashMap<_, _>>();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for concern in &self.concerns {
            let mut unique_dependencies = HashSet::new();
            for dependency in &concern.dependency_concern_ids {
                if dependency == &concern.concern_id
                    || !ids.contains(dependency.as_str())
                    || !unique_dependencies.insert(dependency.as_str())
                {
                    return false;
                }
                let Some(value) = indegree.get_mut(concern.concern_id.as_str()) else {
                    return false;
                };
                *value = value.saturating_add(1);
                dependents
                    .entry(dependency.as_str())
                    .or_default()
                    .push(concern.concern_id.as_str());
            }
        }
        let mut ready = indegree
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect::<VecDeque<_>>();
        let mut visited = 0_usize;
        while let Some(id) = ready.pop_front() {
            visited = visited.saturating_add(1);
            for dependent in dependents.get(id).into_iter().flatten() {
                let Some(count) = indegree.get_mut(dependent) else {
                    return false;
                };
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ready.push_back(dependent);
                }
            }
        }
        visited == self.concerns.len()
    }

    fn active_concurrency_is_valid(&self) -> bool {
        let mut read_compute_count = 0_u16;
        let mut write_resources = HashSet::new();
        let mut substrate_families = HashSet::new();
        for concern in self
            .concerns
            .iter()
            .filter(|concern| concern.status == BeingConcernStatusV1::Active)
        {
            match concern.work_class {
                VolitionWorkClassV1::ReadCompute => {
                    let Some(next) = read_compute_count.checked_add(1) else {
                        return false;
                    };
                    read_compute_count = next;
                },
                VolitionWorkClassV1::ResourceWrite => {
                    if concern
                        .resource_keys
                        .iter()
                        .any(|resource| !write_resources.insert(resource.as_str()))
                    {
                        return false;
                    }
                },
                VolitionWorkClassV1::SubstrateMutation => {
                    if concern
                        .substrate_family
                        .as_deref()
                        .is_some_and(|family| !substrate_families.insert(family))
                    {
                        return false;
                    }
                },
            }
        }
        read_compute_count <= self.max_concurrent_read_compute
    }
}
