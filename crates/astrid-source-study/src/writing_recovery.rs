//! Private, typed recovery. Commands are offers, never automatically executed.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WritingRecovery {
    pub code: String,
    pub draft: Option<String>,
    pub revision: Option<u64>,
    pub commands: Vec<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<WritingCapacity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WritingCapacity {
    pub complete_bytes: usize,
    pub allowance_bytes: usize,
    pub prose_bytes: usize,
    pub reference_bytes: usize,
    pub framing_bytes: usize,
}

impl std::fmt::Display for WritingRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.message, self.commands.join("; "))
    }
}
impl std::error::Error for WritingRecovery {}

/// Storage-only classification, shared by host schedulers before admission.
#[must_use]
pub fn writing_storage_only(action: &str) -> bool {
    let mut words = action.split_whitespace();
    words.next() == Some("WRITE")
        && words.next().is_some_and(|v| {
            matches!(
                v.to_ascii_uppercase().as_str(),
                "FINISH" | "PARK" | "PROFILE" | "QUESTION" | "EVIDENCE" | "STOPPING_POINT"
            )
        })
}

impl WritingRecovery {
    pub(crate) fn finished(id: &str, revision: u64) -> Self {
        Self { code: "draft_finished".into(), draft: Some(id.into()), revision: Some(revision),
            commands: vec![format!("WRITE RESUME {id}"), format!("WRITE READ {id} 1"), "WRITE HELP".into()],
            message: "The draft is finished and unchanged. Explicit resume reopens it; continuation alone cannot.".into(), capacity: None }
    }
}
