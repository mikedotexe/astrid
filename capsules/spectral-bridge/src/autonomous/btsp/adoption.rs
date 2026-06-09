use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ActiveSovereigntyProposal;
use super::helpers::now_unix_s;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(in crate::autonomous) struct ExactAdoption {
    pub owner: String,
    pub response_id: String,
    pub raw_choice: String,
    pub normalized_choice: String,
    #[serde(default)]
    pub context: Option<Value>,
    pub adopted_at_unix_s: u64,
}

impl ExactAdoption {
    pub(super) fn new(
        owner: &str,
        response_id: &str,
        raw_choice: &str,
        normalized_choice: &str,
        context: Option<Value>,
    ) -> Self {
        Self {
            owner: owner.to_string(),
            response_id: response_id.to_string(),
            raw_choice: raw_choice.to_string(),
            normalized_choice: normalized_choice.to_string(),
            context,
            adopted_at_unix_s: now_unix_s(),
        }
    }
}

pub(super) fn record_exact_adoption(
    proposal: &mut ActiveSovereigntyProposal,
    adoption: ExactAdoption,
) {
    if proposal.selected_response_id.is_none() {
        proposal.selected_response_id = Some(adoption.response_id.clone());
    }
    proposal.latest_selected_response_id = Some(adoption.response_id.clone());
    proposal
        .selected_response_ids_by_owner
        .insert(adoption.owner.clone(), adoption.response_id.clone());
    proposal.exact_adoptions.push(adoption);
}

pub(super) fn exact_adoptions_for_scoring(
    proposal: &ActiveSovereigntyProposal,
) -> Vec<ExactAdoption> {
    if !proposal.exact_adoptions.is_empty() {
        return proposal.exact_adoptions.clone();
    }
    proposal
        .selected_response_ids_by_owner
        .iter()
        .map(|(owner, response_id)| ExactAdoption {
            owner: owner.clone(),
            response_id: response_id.clone(),
            raw_choice: response_id.clone(),
            normalized_choice: response_id.clone(),
            context: proposal.adoption_contexts.get(owner).cloned(),
            adopted_at_unix_s: now_unix_s(),
        })
        .collect()
}
