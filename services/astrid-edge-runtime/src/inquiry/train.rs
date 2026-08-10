//! Pure schemas and grammar for the authored inquiry train.
//!
//! This module carries no filesystem, model, reservoir, or deployment authority. The
//! action executor parses these values; the autonomy transaction validates them against
//! the current v7 projection before making any durable transition.

use serde::{Deserialize, Serialize};

use crate::trace::IpcTraceContextV1;

pub(crate) const MAX_INQUIRY_ID_CHARS: usize = 96;
/// One active thread plus at most twelve open/paused (parked) threads.
pub(crate) const MAX_PARKED_INQUIRY_THREADS: usize = 12;
pub(crate) const MAX_INQUIRY_THREADS: usize = MAX_PARKED_INQUIRY_THREADS + 1;
pub(crate) const MAX_THREAD_STARTS_PER_DAY: u32 = 8;
pub(crate) const MAX_BELIEF_EVIDENCE_IDS: usize = 6;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BeliefDisposition {
    Supported,
    Weakened,
    Revised,
    Suspended,
    Unresolved,
}

impl BeliefDisposition {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Weakened => "weakened",
            Self::Revised => "revised",
            Self::Suspended => "suspended",
            Self::Unresolved => "unresolved",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ThreadAction {
    Open {
        question: String,
    },
    Branch {
        thread_id: String,
        question: String,
    },
    Resume {
        thread_id: String,
    },
    Pause {
        thread_id: String,
        reason: String,
    },
    Close {
        thread_id: String,
        conclusion: String,
    },
    UpdateBelief {
        belief_id: String,
        evidence_ids: Vec<String>,
        disposition: BeliefDisposition,
        claim: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InquiryThreadOperation {
    Continue,
    Open,
    Branch,
    Pause,
    Close,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InquiryBeliefOperation {
    Unchanged,
    Propose,
    Support,
    Weaken,
    Revise,
    Suspend,
    Resolve,
}

/// A fully verified steward projection. Construction by the mutable runtime does not
/// establish authenticity: the immutable admission boundary must first verify the signed
/// ledger/reflection join, then map that bounded result into this type.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedInquiryStepInput {
    pub(crate) step_id: String,
    pub(crate) entry_hash: String,
    pub(crate) mechanical_predecessor_hash: Option<String>,
    pub(crate) ledger_segment: u64,
    pub(crate) ledger_entry_index: u64,
    pub(crate) parent_step_id: Option<String>,
    pub(crate) thread_operation: InquiryThreadOperation,
    pub(crate) thread_id: Option<String>,
    pub(crate) observation: String,
    pub(crate) interpretation: String,
    pub(crate) uncertainty: String,
    pub(crate) decision: String,
    pub(crate) counterpoint: Option<String>,
    pub(crate) next_test: Option<String>,
    pub(crate) evidence_ids: Vec<String>,
    pub(crate) confidence: String,
    pub(crate) belief_operation: Option<InquiryBeliefOperation>,
    pub(crate) belief_id: Option<String>,
    pub(crate) belief_claim: Option<String>,
    pub(crate) trigger: String,
    pub(crate) recorded_at_unix_ms: u64,
    pub(crate) trace: IpcTraceContextV1,
    pub(crate) response_sha256: String,
    pub(crate) reflection_sha256: String,
    pub(crate) declaration_sha256: String,
}

/// Result of joining one cryptographically verified steward entry into mutable
/// working continuity. Verification progress and semantic admission are
/// intentionally orthogonal: a rejected signed step advances only the former.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InquiryAdmissionOutcome {
    Admitted {
        summary: String,
        verified_gap: bool,
    },
    AdmittedReplay,
    SemanticallyRejected {
        step_id: String,
        reason: String,
        verified_gap: bool,
    },
    RejectedReplay {
        step_id: String,
        reason: String,
    },
}

impl InquiryAdmissionOutcome {
    pub(crate) const fn reservoir_eligible(&self) -> bool {
        matches!(self, Self::Admitted { .. } | Self::AdmittedReplay)
    }
}

pub(crate) fn parse_thread_action(verb: &str, argument: &str) -> Option<ThreadAction> {
    match verb {
        "OPEN_THREAD" => bounded_text(argument).map(|question| ThreadAction::Open { question }),
        "BRANCH_THREAD" => {
            let (thread_id, question) = argument.split_once("::")?;
            Some(ThreadAction::Branch {
                thread_id: bounded_identifier(thread_id)?,
                question: bounded_text(question)?,
            })
        },
        "RESUME_THREAD" => {
            bounded_identifier(argument).map(|thread_id| ThreadAction::Resume { thread_id })
        },
        "PAUSE_THREAD" => {
            let (thread_id, reason) = argument.split_once("::")?;
            Some(ThreadAction::Pause {
                thread_id: bounded_identifier(thread_id)?,
                reason: bounded_text(reason)?,
            })
        },
        "CLOSE_THREAD" => {
            let (thread_id, conclusion) = argument.split_once("::")?;
            Some(ThreadAction::Close {
                thread_id: bounded_identifier(thread_id)?,
                conclusion: bounded_text(conclusion)?,
            })
        },
        "UPDATE_BELIEF" => parse_update_belief(argument),
        _ => None,
    }
}

fn parse_update_belief(argument: &str) -> Option<ThreadAction> {
    let upper = argument.to_ascii_uppercase();
    let with = upper.find(" WITH ")?;
    let belief_id = bounded_identifier(argument.get(..with)?)?;
    let after_with = argument.get(with.saturating_add(" WITH ".len())..)?;
    let (evidence, remainder) = after_with.split_once("::")?;
    let (disposition, claim) = remainder.split_once("::")?;
    let disposition = match disposition.trim().to_ascii_lowercase().as_str() {
        "supported" => BeliefDisposition::Supported,
        "weakened" => BeliefDisposition::Weakened,
        "revised" => BeliefDisposition::Revised,
        "suspended" => BeliefDisposition::Suspended,
        "unresolved" => BeliefDisposition::Unresolved,
        _ => return None,
    };
    let mut evidence_ids = Vec::new();
    for identifier in evidence.split(',') {
        let identifier = bounded_identifier(identifier)?;
        if evidence_ids.contains(&identifier) {
            return None;
        }
        evidence_ids.push(identifier);
    }
    if evidence_ids.is_empty() || evidence_ids.len() > MAX_BELIEF_EVIDENCE_IDS {
        return None;
    }
    Some(ThreadAction::UpdateBelief {
        belief_id,
        evidence_ids,
        disposition,
        claim: bounded_text(claim)?,
    })
}

pub(crate) fn bounded_identifier(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_INQUIRY_ID_CHARS
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return None;
    }
    Some(value.to_string())
}

fn bounded_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 2_000 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{BeliefDisposition, ThreadAction, bounded_identifier, parse_thread_action};

    #[test]
    fn parses_exact_thread_lifecycle_and_belief_actions() {
        assert!(matches!(
            parse_thread_action("OPEN_THREAD", "What changed?"),
            Some(ThreadAction::Open { .. })
        ));
        assert!(matches!(
            parse_thread_action("BRANCH_THREAD", "thread-1 :: A narrower question"),
            Some(ThreadAction::Branch { .. })
        ));
        assert_eq!(
            parse_thread_action(
                "UPDATE_BELIEF",
                "belief-1 WITH source_1.md,study_2.md :: revised :: A narrower claim"
            ),
            Some(ThreadAction::UpdateBelief {
                belief_id: "belief-1".to_string(),
                evidence_ids: vec!["source_1.md".to_string(), "study_2.md".to_string()],
                disposition: BeliefDisposition::Revised,
                claim: "A narrower claim".to_string(),
            })
        );
    }

    #[test]
    fn identifiers_are_basename_safe_and_bounded() {
        assert!(bounded_identifier("thread-1.alpha").is_some());
        assert!(bounded_identifier("../thread").is_none());
        assert!(bounded_identifier("nested/thread").is_none());
        assert!(bounded_identifier(&"x".repeat(97)).is_none());
        assert!(parse_thread_action("RESUME_THREAD", "../outside").is_none());
    }
}
