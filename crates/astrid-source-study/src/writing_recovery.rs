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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_capacity: Option<FinalWritingCapacity>,
}

/// Byte counts from final provider admission, never private prose or wire data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FinalWritingCapacity {
    pub prepared_input_bytes: usize,
    pub system_bytes: usize,
    pub wrapper_bytes: usize,
    pub allowance_bytes: usize,
}

impl FinalWritingCapacity {
    #[must_use]
    pub fn complete_bytes(&self) -> Option<usize> {
        self.prepared_input_bytes
            .checked_add(self.system_bytes)?
            .checked_add(self.wrapper_bytes)
    }
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
    /// Replace ineffective retry advice only for the still-valid pending input.
    /// # Errors
    /// Rejects inconsistent or overflowing final-request byte counts.
    pub(crate) fn with_final_capacity(
        mut self,
        capacity: FinalWritingCapacity,
    ) -> anyhow::Result<Self> {
        let complete = capacity
            .complete_bytes()
            .ok_or_else(|| anyhow::anyhow!("invalid final writing byte counts"))?;
        anyhow::ensure!(
            capacity.allowance_bytes > 0 && complete > capacity.allowance_bytes,
            "final writing request is not over its allowance"
        );
        if self.code != "pending_generation_available" {
            return Ok(self);
        }
        let id = self
            .draft
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("pending draft identity missing"))?;
        self.code = "final_request_too_large".into();
        self.commands = vec![
            format!("WRITE READ {id} 1"),
            "WRITE PARK".into(),
            "WRITE HELP".into(),
        ];
        self.message = format!(
            "Final protected writing needs {complete} UTF-8 message-content bytes; this provider lane allows {}. Prepared input: {}; adapted system: {}; source wrapper: {}. The complete draft and pending input are preserved. Repeating the unchanged request cannot fix this limit. Inspect or park this draft; WRITE HELP also lists new-writing choices.",
            capacity.allowance_bytes,
            capacity.prepared_input_bytes,
            capacity.system_bytes,
            capacity.wrapper_bytes
        );
        self.final_capacity = Some(capacity);
        Ok(self)
    }

    pub(crate) fn finished(id: &str, revision: u64) -> Self {
        Self { code: "draft_finished".into(), draft: Some(id.into()), revision: Some(revision),
            commands: vec![format!("WRITE RESUME {id}"), format!("WRITE READ {id} 1"), "WRITE HELP".into()],
            message: "The draft is finished and unchanged. Explicit resume reopens it; continuation alone cannot.".into(), capacity: None, final_capacity: None }
    }
}
