//! Operation-local observations of file-first saves. Never a retry or atomicity claim.
use super::*;
use sha2::{Digest as _, Sha256};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WriteObservation {
    #[default]
    NotAttempted,
    ReturnedOk,
    FailedOutcomeUnknown,
    NotRequested,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ThreadSaveReceipt {
    projection: WriteObservation,
    thread_json: WriteObservation,
    next_md: WriteObservation,
    index_read: WriteObservation,
    index_write: WriteObservation,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SaveReceipt {
    schema: &'static str,
    thread_id: String,
    record_kind: &'static str,
    record_id: String,
    record_sha256: String,
    history_append: WriteObservation,
    thread: ThreadSaveReceipt,
    database_mirror: WriteObservation,
    failed_step: Option<&'static str>,
    error_kind: Option<String>,
    retry_authorized: bool,
    guarantee: &'static str,
}

impl SaveReceipt {
    pub(super) fn new(
        thread: &ResearchThread,
        kind: &'static str,
        id: &str,
        record: &impl Serialize,
    ) -> Result<Self> {
        Ok(Self {
            schema: "continuity_save_receipt_v1",
            thread_id: thread.thread_id.clone(),
            record_kind: kind,
            record_id: id.into(),
            record_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(record)?)),
            history_append: WriteObservation::NotAttempted,
            thread: ThreadSaveReceipt::default(),
            database_mirror: WriteObservation::NotAttempted,
            failed_step: None,
            error_kind: None,
            retry_authorized: false,
            guarantee: "returned_ok reports a completed call, not fsync, atomic commit, execution, or restored understanding. failed_outcome_unknown may include partial bytes. Inspect retained history and selection before any retry; this receipt never authorizes replay.",
        })
    }

    fn failure(&mut self, step: &'static str, error: &anyhow::Error) {
        self.failed_step = Some(step);
        self.error_kind = Some(error.downcast_ref::<std::io::Error>().map_or_else(
            || "operation_failed".into(),
            |error| format!("{:?}", error.kind()),
        ));
    }

    pub(super) fn append_failed(mut self, error: &anyhow::Error) -> anyhow::Error {
        self.history_append = WriteObservation::FailedOutcomeUnknown;
        self.failure("history_append", error);
        SaveError(self).into()
    }

    pub(super) fn appended(&mut self) {
        self.history_append = WriteObservation::ReturnedOk;
    }
}

#[derive(Debug)]
struct SaveError(SaveReceipt);

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Continuity save did not complete. Retained state may have advanced; no automatic retry. {}",
            serde_json::to_string(&self.0).map_err(|_| std::fmt::Error)?
        )
    }
}
impl std::error::Error for SaveError {}

#[derive(Debug)]
struct ThreadSaveError {
    receipt: ThreadSaveReceipt,
    step: &'static str,
    kind: String,
}
impl std::fmt::Display for ThreadSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Thread save incomplete at {}; outcome of failed writes is unknown; no automatic retry. {}",
            self.step,
            serde_json::to_string(&self.receipt).map_err(|_| std::fmt::Error)?
        )
    }
}
impl std::error::Error for ThreadSaveError {}

pub(super) fn is_save_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<SaveError>().is_some() || error.downcast_ref::<ThreadSaveError>().is_some()
}

#[derive(Clone, Debug, Default)]
pub(super) struct SaveWarnings(Arc<Mutex<WarningBatch>>);

#[derive(Debug, Default)]
struct WarningBatch {
    receipts: Vec<SaveReceipt>,
    omitted: usize,
}

impl ActionContinuityStore {
    pub(super) fn with_save_warnings(mut self) -> Self {
        self.save_warnings = Some(SaveWarnings::default());
        self
    }

    pub(super) fn finish_save_warnings(&self, result: Result<String>) -> Result<String> {
        let Some(warnings) = &self.save_warnings else {
            return result;
        };
        let batch = std::mem::take(
            &mut *warnings
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        if batch.receipts.is_empty() {
            return result;
        }
        let text = format!(
            "Save warnings (metadata only; no retry authorized): {}",
            json!({
                "receipts":batch.receipts, "omitted_warning_count":batch.omitted
            })
        );
        match result {
            Ok(message) => Ok(format!("{message}\n\n{text}")),
            Err(error) => Err(error.context(text)),
        }
    }

    pub(super) fn save_thread_and_mirror(
        &self,
        db: Option<&BridgeDb>,
        thread: &ResearchThread,
        mut receipt: SaveReceipt,
    ) -> Result<()> {
        if db.is_none() {
            receipt.database_mirror = WriteObservation::NotRequested;
        }
        match self.write_thread_observed(thread) {
            Ok(observed) => receipt.thread = observed,
            Err(error) => {
                if let Some(observed) = error.downcast_ref::<ThreadSaveError>() {
                    receipt.thread = observed.receipt.clone();
                    receipt.failed_step = Some(observed.step);
                    receipt.error_kind = Some(observed.kind.clone());
                } else {
                    receipt.failure("thread_save", &error);
                }
                return Err(SaveError(receipt).into());
            },
        }
        let Some(db) = db else { return Ok(()) };
        // Keep the mirror best-effort: do not suppress later work or replay a saved append.
        let result = serde_json::to_string(thread)
            .map_err(anyhow::Error::from)
            .and_then(|payload| db.mirror_action_thread(&thread.thread_id, &payload));
        if let Err(error) = result {
            receipt.database_mirror = WriteObservation::FailedOutcomeUnknown;
            receipt.failure("database_mirror", &error);
            tracing::warn!(save_receipt = %serde_json::to_string(&receipt)?, "Continuity file save completed; database mirror unavailable");
            if let Some(warnings) = &self.save_warnings {
                // One synchronous command owns this collection; nothing crosses an inference/job.
                let mut batch = warnings
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if batch.receipts.len() < 8 {
                    batch.receipts.push(receipt);
                } else {
                    batch.omitted = batch.omitted.saturating_add(1);
                }
            }
        }
        Ok(())
    }

    pub(super) fn write_thread_observed(
        &self,
        thread: &ResearchThread,
    ) -> Result<ThreadSaveReceipt> {
        let mut receipt = ThreadSaveReceipt::default();
        let mut thread = thread.clone();
        macro_rules! step {
            ($field:ident, $operation:expr) => {
                if let Err(error) = $operation {
                    receipt.$field = WriteObservation::FailedOutcomeUnknown;
                    return Err(ThreadSaveError {
                        receipt,
                        step: stringify!($field),
                        kind: error.downcast_ref::<std::io::Error>().map_or_else(
                            || "operation_failed".into(),
                            |error| format!("{:?}", error.kind()),
                        ),
                    }
                    .into());
                }
                receipt.$field = WriteObservation::ReturnedOk;
            };
        }
        step!(
            projection,
            self.refresh_projection_freshness_v1(&mut thread, "write_thread")
        );
        step!(
            thread_json,
            self.write_json(
                &self.thread_dir(&thread.thread_id).join("thread.json"),
                &thread
            )
        );
        step!(next_md, self.write_next_md(&thread));
        let mut index = None;
        step!(
            index_read,
            self.load_index().map(|value| index = Some(value))
        );
        let mut index = index.context("observed index read returned no value")?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        step!(index_write, self.save_index(&index));
        Ok(receipt)
    }
}

#[cfg(test)]
pub(super) fn receipt_from_error(error: &anyhow::Error) -> Value {
    serde_json::to_value(
        &error
            .downcast_ref::<SaveError>()
            .expect("typed save error")
            .0,
    )
    .unwrap()
}
