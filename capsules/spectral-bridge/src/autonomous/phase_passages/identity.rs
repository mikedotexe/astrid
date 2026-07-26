use serde_json::Value;

use super::{latest_passage, reduce_passages};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::autonomous) struct PassageIdentityV1 {
    pub passage_id: String,
    pub transition_id: String,
    pub actor: String,
}

pub(in crate::autonomous) fn resolve_passage_identity(
    records: &[Value],
    selector: &str,
    actor: &str,
) -> Result<PassageIdentityV1, String> {
    let (passages, errors) = reduce_passages(records);
    if !errors.is_empty() {
        return Err(format!(
            "passage history has {} invalid row(s)",
            errors.len()
        ));
    }
    let passage = latest_passage(&passages, selector, actor)
        .ok_or_else(|| "no matching self-authored passage".to_string())?;
    Ok(PassageIdentityV1 {
        passage_id: passage.passage_id.clone(),
        transition_id: passage.transition_id.clone(),
        actor: passage.actor.clone(),
    })
}
