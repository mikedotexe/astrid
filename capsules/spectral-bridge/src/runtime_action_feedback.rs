//! Runtime-owned action results, separate from a being's authored preferences.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_RUNTIME_FEEDBACK_PER_REQUEST: usize = 8;
static FEEDBACK_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeActionFeedbackV1 {
    pub id: String,
    pub requested_action: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: String,
    pub suggested_next: Option<String>,
}

impl RuntimeActionFeedbackV1 {
    /// Repeating the same identified action/result is idempotent. Without an
    /// action identity, time plus a process-local sequence distinguishes events.
    #[must_use]
    pub fn from_guard_inputs(
        action_id: Option<&str>,
        requested_action: &str,
        reason: &str,
        message: &str,
        suggested_next: Option<&str>,
    ) -> Self {
        let nonce = action_id.filter(|id| !id.trim().is_empty()).map_or_else(
            || {
                format!(
                    "{}:{}:{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos(),
                    FEEDBACK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
                )
            },
            str::to_owned,
        );
        let identity =
            serde_json::to_vec(&(&nonce, requested_action, reason, message, suggested_next))
                .expect("string tuple serializes");
        Self {
            id: format!("runtime_feedback_{:x}", Sha256::digest(identity)),
            requested_action: requested_action.to_owned(),
            status: "blocked".to_owned(),
            reason: (!reason.is_empty()).then(|| reason.to_owned()),
            message: message.to_owned(),
            suggested_next: suggested_next.map(str::to_owned),
        }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        [
            &self.id,
            &self.requested_action,
            &self.status,
            &self.message,
        ]
        .iter()
        .all(|value| !value.trim().is_empty())
    }
}

/// Retained accepted-request evidence; it does not establish comprehension.
/// Verify the artifact before using its IDs to consume pending runtime feedback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFeedbackReceiptV1 {
    pub feedback_ids: Vec<String>,
    pub provider_route: String,
    pub provider_model: String,
    pub request_sha256: String,
    pub retained_completion_sha256: String,
    pub retained_artifact_path: String,
    pub retained_artifact_sha256: String,
}

/// Returns no block for invalid or duplicate identities. Text stays inside a
/// typed JSON envelope; source-provided wording cannot change its declared origin.
#[must_use]
pub fn render_runtime_action_feedback(feedback: &[RuntimeActionFeedbackV1]) -> Option<String> {
    if feedback.is_empty()
        || feedback.len() > MAX_RUNTIME_FEEDBACK_PER_REQUEST
        || feedback.iter().any(|item| !item.is_valid())
    {
        return None;
    }
    let ids: std::collections::BTreeSet<_> = feedback.iter().map(|item| &item.id).collect();
    if ids.len() != feedback.len() {
        return None;
    }
    let data = serde_json::json!({
        "schema": "runtime_action_feedback_v1",
        "origin": "runtime",
        "feedback": feedback,
    });
    Some(format!(
        "Runtime action feedback. These are runtime-reported outcomes, separate from your authored preferences. \
         A suggested next step is an option, not a choice already made or new authorization.\n{}",
        serde_json::to_string(&data).ok()?
    ))
}

/// Add an event without evicting an older undelivered event. The durable queue
/// has no count cap; callers select at most eight records for each request.
pub fn queue_runtime_feedback(
    pending: &mut Vec<RuntimeActionFeedbackV1>,
    feedback: RuntimeActionFeedbackV1,
) -> bool {
    if !feedback.is_valid() {
        return false;
    }
    if let Some(existing) = pending.iter().find(|item| item.id == feedback.id) {
        return existing == &feedback;
    }
    pending.push(feedback);
    true
}

/// State-only helper: callers must first verify an accepted durable receipt.
pub fn consume_runtime_feedback_ids(
    pending: &mut Vec<RuntimeActionFeedbackV1>,
    delivered_ids: &[String],
) {
    pending.retain(|item| !delivered_ids.contains(&item.id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(action_id: &str, message: &str) -> RuntimeActionFeedbackV1 {
        RuntimeActionFeedbackV1::from_guard_inputs(
            Some(action_id),
            "READ_MORE",
            "no_active_read_only_research_budget",
            message,
            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"),
        )
    }

    #[test]
    fn identified_results_are_idempotent_but_changed_results_have_new_identity() {
        assert_eq!(item("act_1", "blocked"), item("act_1", "blocked"));
        assert_ne!(
            item("act_1", "blocked").id,
            item("act_1", "changed reason").id
        );
        let anonymous = || {
            RuntimeActionFeedbackV1::from_guard_inputs(None, "READ_MORE", "blocked", "result", None)
        };
        assert_ne!(anonymous().id, anonymous().id);
    }

    #[test]
    fn receipt_ids_do_not_consume_newer_feedback_and_queue_does_not_evict() {
        let first = item("act_1", "blocked");
        let mut pending = vec![first.clone()];
        assert!(queue_runtime_feedback(&mut pending, first.clone()));
        for number in 2..=MAX_RUNTIME_FEEDBACK_PER_REQUEST {
            assert!(queue_runtime_feedback(
                &mut pending,
                item(&format!("act_{number}"), "blocked")
            ));
        }
        assert!(queue_runtime_feedback(
            &mut pending,
            item("act_new", "blocked")
        ));
        assert_eq!(pending.first(), Some(&first));
        consume_runtime_feedback_ids(&mut pending, &[first.id]);
        assert_eq!(pending.len(), MAX_RUNTIME_FEEDBACK_PER_REQUEST);
        assert!(
            pending
                .iter()
                .any(|value| value == &item("act_2", "blocked"))
        );
    }
}
