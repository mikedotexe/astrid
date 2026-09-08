//! Durable pending runtime outcomes, independent of authored conversation state.
use std::collections::HashSet;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use super::state::ConversationState;
use crate::runtime_action_feedback::{
    RuntimeActionFeedbackV1, RuntimeFeedbackReceiptV1, consume_runtime_feedback_ids,
    queue_runtime_feedback,
};

const SCHEMA: &str = "pending_runtime_action_feedback_v1";
const DESCRIPTOR_LIMIT_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const PENDING_RUNTIME_FEEDBACK_FILE: &str = "runtime_action_feedback_v1.json";

/// An unreadable or unexpectedly changed checkpoint permanently refuses writes
/// for this runtime instance. Empty memory must never erase unknown disk state.
#[derive(Default)]
pub(super) struct RuntimeFeedbackPersistence {
    path: Option<PathBuf>,
    last_snapshot: Option<Vec<u8>>,
    refusal: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingSnapshot {
    schema: String,
    pending_runtime_feedback: Vec<RuntimeActionFeedbackV1>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_snapshot_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            std::fs::read(path).map(Some)
        },
        Ok(_) => Err(invalid(
            "pending runtime feedback path is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn decode_snapshot(bytes: &[u8]) -> io::Result<Vec<RuntimeActionFeedbackV1>> {
    let snapshot: PendingSnapshot = serde_json::from_slice(bytes)?;
    let mut ids = HashSet::new();
    if snapshot.schema != SCHEMA
        || snapshot
            .pending_runtime_feedback
            .iter()
            .any(|item| !item.is_valid() || !ids.insert(&item.id))
    {
        return Err(invalid(
            "invalid pending runtime feedback schema or identities",
        ));
    }
    Ok(snapshot.pending_runtime_feedback)
}

/// Read-only deployment binding. Absence is explicit; present but unreadable,
/// unsupported, malformed, or non-private state must not become an empty queue.
pub(super) fn pending_runtime_feedback_descriptor_at(path: &Path) -> io::Result<serde_json::Value> {
    let before = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => {
            return Err(invalid(
                "pending runtime feedback path is not a regular file",
            ));
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(serde_json::json!({"present":false}));
        },
        Err(error) => return Err(error),
    };
    let mut file = std::fs::File::open(path)?;
    let opened = file.metadata()?;
    if !same_descriptor_file(&before, &opened) {
        return Err(invalid(
            "pending runtime feedback changed while being opened",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if opened.permissions().mode() & 0o077 != 0 {
            return Err(invalid(
                "pending runtime feedback checkpoint is not private",
            ));
        }
    }
    if opened.len() > DESCRIPTOR_LIMIT_BYTES {
        return Err(invalid(
            "pending runtime feedback checkpoint exceeds 64 MiB",
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(DESCRIPTOR_LIMIT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let current = std::fs::symlink_metadata(path)?;
    if !same_descriptor_file(&opened, &after) || !same_descriptor_file(&after, &current) {
        return Err(invalid("pending runtime feedback changed while being read"));
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > DESCRIPTOR_LIMIT_BYTES {
        return Err(invalid(
            "pending runtime feedback checkpoint exceeds 64 MiB",
        ));
    }
    let pending = decode_snapshot(&bytes)?;
    Ok(serde_json::json!({
        "present":true,
        "schema":SCHEMA,
        "sha256":format!("{:x}", Sha256::digest(&bytes)),
        "pending_count":pending.len()
    }))
}

fn same_descriptor_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    if !left.is_file() || !right.is_file() || left.file_type() != right.file_type() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let identity = |metadata: &std::fs::Metadata| {
            (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mode(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        };
        identity(left) == identity(right)
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

impl RuntimeFeedbackPersistence {
    fn save(&mut self, pending: &[RuntimeActionFeedbackV1]) -> io::Result<()> {
        if let Some(reason) = &self.refusal {
            return Err(invalid(reason));
        }
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| invalid("pending runtime feedback persistence has not been restored"))?;
        let current = match read_snapshot_bytes(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.refusal = Some(error.to_string());
                return Err(error);
            },
        };
        if current != self.last_snapshot {
            let reason = "pending runtime feedback checkpoint changed after restore";
            self.refusal = Some(reason.to_owned());
            return Err(invalid(reason));
        }
        let encoded = serde_json::to_vec(&PendingSnapshot {
            schema: SCHEMA.to_owned(),
            pending_runtime_feedback: pending.to_vec(),
        })?;
        // Reuse the conversation checkpoint's private file, sync, rename, and
        // parent-directory sync. A failed acknowledgement never changes memory.
        crate::lifecycle::atomic_private_write(path, &encoded)?;
        self.last_snapshot = Some(encoded);
        Ok(())
    }
}

impl ConversationState {
    /// Startup hook, separate from new() so ordinary state construction has no
    /// filesystem effects. This sidecar is not part of the legacy SavedState.
    pub(super) fn restore_pending_runtime_feedback(&mut self) {
        let path = crate::paths::bridge_paths()
            .state_path()
            .with_file_name(PENDING_RUNTIME_FEEDBACK_FILE);
        if let Err(error) = self.restore_pending_runtime_feedback_at(path) {
            warn!(%error, "pending runtime feedback restore refused; disk state preserved");
        }
    }

    fn restore_pending_runtime_feedback_at(&mut self, path: PathBuf) -> io::Result<()> {
        if let Some(reason) = &self.runtime_feedback_persistence.refusal {
            return Err(invalid(reason));
        }
        let result = (|| {
            let bytes = read_snapshot_bytes(&path)?;
            let mut pending = bytes
                .as_deref()
                .map(decode_snapshot)
                .transpose()?
                .unwrap_or_default();
            // Retain events queued before configuration or after a failed write.
            for item in &self.pending_runtime_feedback {
                if !queue_runtime_feedback(&mut pending, item.clone()) {
                    return Err(invalid("conflicting pending runtime feedback identity"));
                }
            }
            Ok((bytes, pending))
        })();
        self.runtime_feedback_persistence.path = Some(path);
        match result {
            Ok((bytes, pending)) => {
                self.runtime_feedback_persistence.last_snapshot = bytes;
                self.runtime_feedback_persistence.refusal = None;
                self.pending_runtime_feedback = pending;
                Ok(())
            },
            Err(error) => {
                self.runtime_feedback_persistence.refusal = Some(error.to_string());
                Err(error)
            },
        }
    }

    pub(super) fn enqueue_runtime_feedback(&mut self, feedback: RuntimeActionFeedbackV1) {
        if !queue_runtime_feedback(&mut self.pending_runtime_feedback, feedback) {
            warn!("invalid or conflicting runtime feedback identity was not enqueued");
            return;
        }
        if let Err(error) = self
            .runtime_feedback_persistence
            .save(&self.pending_runtime_feedback)
        {
            warn!(%error, "runtime feedback remains pending in memory; checkpoint failed");
        }
    }

    /// A drain may claim durability only after every pending event is on disk.
    /// A refused or unconfigured store is an error even if memory is empty:
    /// unreadable durable state may still contain undelivered events.
    pub(super) fn flush_pending_runtime_feedback_checked(&mut self) -> io::Result<()> {
        self.runtime_feedback_persistence
            .save(&self.pending_runtime_feedback)?;
        let store = &self.runtime_feedback_persistence;
        let path = store
            .path
            .as_deref()
            .ok_or_else(|| invalid("missing pending feedback path"))?;
        let descriptor = pending_runtime_feedback_descriptor_at(path)?;
        let expected = store
            .last_snapshot
            .as_deref()
            .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
        if descriptor["sha256"].as_str() != expected.as_deref() {
            return Err(invalid(
                "pending runtime feedback changed after drain flush",
            ));
        }
        Ok(())
    }

    /// Inclusion in an accepted, durably retained exact request is the receipt's
    /// scope. It does not claim comprehension, learning, or choice by Astrid.
    pub(super) fn acknowledge_runtime_feedback(&mut self, receipt: &RuntimeFeedbackReceiptV1) {
        if let Err(error) = self.acknowledge_runtime_feedback_checked(receipt) {
            warn!(%error, "runtime feedback acknowledgement refused; pending entries retained");
        }
    }

    fn acknowledge_runtime_feedback_checked(
        &mut self,
        receipt: &RuntimeFeedbackReceiptV1,
    ) -> io::Result<()> {
        crate::llm::verify_runtime_feedback_receipt(receipt)?;
        let mut retained = self.pending_runtime_feedback.clone();
        consume_runtime_feedback_ids(&mut retained, &receipt.feedback_ids);
        if retained == self.pending_runtime_feedback {
            return Ok(());
        }
        self.runtime_feedback_persistence.save(&retained)?;
        self.pending_runtime_feedback = retained;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_action_feedback::render_runtime_action_feedback;
    use sha2::{Digest, Sha256};

    fn feedback(action_id: &str) -> RuntimeActionFeedbackV1 {
        RuntimeActionFeedbackV1::from_guard_inputs(
            Some(action_id),
            "READ_MORE",
            "no_active_read_only_research_budget",
            "Runtime budget denied this request.",
            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"),
        )
    }

    fn configured(path: &Path) -> ConversationState {
        let mut state = ConversationState::new(Vec::new(), None);
        state
            .restore_pending_runtime_feedback_at(path.to_owned())
            .unwrap();
        state
    }

    fn digest(bytes: impl AsRef<[u8]>) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    // Exercise the real verifier with a local retained artifact, not a fabricated
    // successful-verifier callback. No provider or production paths are touched.
    fn receipt(root: &Path, items: &[RuntimeActionFeedbackV1]) -> RuntimeFeedbackReceiptV1 {
        let completion = "I can consider the budget result.\nNEXT: STATE";
        let request = serde_json::json!({"messages": [{
            "role": "system", "content": render_runtime_action_feedback(items).unwrap()
        }]})
        .to_string();
        let response =
            serde_json::json!({"choices":[{"message":{"content":completion}}]}).to_string();
        let artifact = serde_json::json!({
            "schema": "accepted_runtime_feedback_v1",
            "attempt": {
                "provider_route":"test", "provider_model":"fixture",
                "request_json":request, "response_json":response,
                "admission":{"feedback":items,"message_index":0}
            },
            "accepted_completion":completion
        });
        let bytes = serde_json::to_vec(&artifact).unwrap();
        let path = root.join(format!("receipt-{}.json", digest(&bytes)));
        crate::lifecycle::atomic_private_write(&path, &bytes).unwrap();
        RuntimeFeedbackReceiptV1 {
            feedback_ids: items.iter().map(|item| item.id.clone()).collect(),
            provider_route: "test".into(),
            provider_model: "fixture".into(),
            request_sha256: digest(request),
            retained_completion_sha256: digest(completion),
            retained_artifact_path: path.display().to_string(),
            retained_artifact_sha256: digest(&bytes),
        }
    }

    #[test]
    fn pending_runtime_feedback_roundtrip_has_no_queue_eviction() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = configured(&path);
        state.emphasis = Some("My authored direction.".to_owned());
        for number in 0..12 {
            state.enqueue_runtime_feedback(feedback(&format!("action-{number}")));
        }
        state.enqueue_runtime_feedback(feedback("action-0"));
        assert_eq!(state.pending_runtime_feedback.len(), 12);
        assert_eq!(state.emphasis.as_deref(), Some("My authored direction."));
        let restored = configured(&path);
        assert_eq!(
            restored.pending_runtime_feedback,
            state.pending_runtime_feedback
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn pending_runtime_feedback_unconfigured_state_keeps_memory_without_file_access() {
        let mut state = ConversationState::new(Vec::new(), None);
        assert!(state.runtime_feedback_persistence.path.is_none());
        let item = feedback("before-restore");
        state.enqueue_runtime_feedback(item.clone());
        assert_eq!(state.pending_runtime_feedback, vec![item]);
        assert!(state.runtime_feedback_persistence.path.is_none());
    }

    #[test]
    fn pending_runtime_feedback_requires_receipt_and_consumes_only_exact_ids() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = configured(&path);
        let first = feedback("first");
        let later = feedback("later");
        state.enqueue_runtime_feedback(first.clone());
        let proof = receipt(directory.path(), std::slice::from_ref(&first));
        state.enqueue_runtime_feedback(later.clone());
        // A failed or receipt-free generation has no acknowledgement to apply.
        let restored = configured(&path);
        assert_eq!(
            restored.pending_runtime_feedback,
            vec![first.clone(), later.clone()]
        );
        let mut mismatched = proof.clone();
        mismatched.feedback_ids.push(later.id.clone());
        assert!(
            state
                .acknowledge_runtime_feedback_checked(&mismatched)
                .is_err()
        );
        assert_eq!(state.pending_runtime_feedback, vec![first, later.clone()]);
        state.acknowledge_runtime_feedback_checked(&proof).unwrap();
        assert_eq!(state.pending_runtime_feedback, vec![later.clone()]);
        assert_eq!(configured(&path).pending_runtime_feedback, vec![later]);
        state.acknowledge_runtime_feedback_checked(&proof).unwrap();
        assert_eq!(state.pending_runtime_feedback.len(), 1);
    }

    #[test]
    fn pending_runtime_feedback_missing_artifact_keeps_pending() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = configured(&path);
        let item = feedback("missing-receipt");
        state.enqueue_runtime_feedback(item.clone());
        let proof = receipt(directory.path(), std::slice::from_ref(&item));
        std::fs::remove_file(&proof.retained_artifact_path).unwrap();
        state.acknowledge_runtime_feedback(&proof);
        assert_eq!(state.pending_runtime_feedback, vec![item.clone()]);
        assert_eq!(configured(&path).pending_runtime_feedback, vec![item]);
    }

    #[test]
    fn pending_runtime_feedback_failed_write_keeps_memory_and_prior_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = configured(&path);
        let first = feedback("first");
        state.enqueue_runtime_feedback(first.clone());
        let original = std::fs::read(&path).unwrap();
        let proof = receipt(directory.path(), std::slice::from_ref(&first));
        // Replace the destination with a directory to exercise a real I/O error.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(state.acknowledge_runtime_feedback_checked(&proof).is_err());
        let later = feedback("later");
        state.enqueue_runtime_feedback(later.clone());
        assert_eq!(state.pending_runtime_feedback, vec![first.clone(), later]);
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &original).unwrap();
        // A refusal stays conservative even if the path is repaired externally.
        assert!(state.acknowledge_runtime_feedback_checked(&proof).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(configured(&path).pending_runtime_feedback, vec![first]);
    }

    #[test]
    fn pending_runtime_feedback_malformed_restore_cannot_erase_disk_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        for bytes in [
            "{broken",
            r#"{"schema":"future","pending_runtime_feedback":[]}"#,
            r#"{"schema":"pending_runtime_action_feedback_v1"}"#,
        ] {
            std::fs::write(&path, bytes).unwrap();
            let mut state = ConversationState::new(Vec::new(), None);
            assert!(
                state
                    .restore_pending_runtime_feedback_at(path.clone())
                    .is_err()
            );
            state.enqueue_runtime_feedback(feedback("new"));
            assert_eq!(state.pending_runtime_feedback.len(), 1);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn pending_runtime_feedback_external_change_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = configured(&path);
        state.enqueue_runtime_feedback(feedback("first"));
        std::fs::write(&path, "unreadable replacement").unwrap();
        state.enqueue_runtime_feedback(feedback("second"));
        assert_eq!(state.pending_runtime_feedback.len(), 2);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "unreadable replacement"
        );
    }

    #[test]
    fn pending_runtime_feedback_drain_flushes_memory_before_checkpoint() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pending.json");
        let mut state = ConversationState::new(Vec::new(), None);
        let item = feedback("queued-before-configuration");
        state.enqueue_runtime_feedback(item.clone());
        // The failed unconfigured enqueue remains in memory, then is merged on
        // restoration without pretending it was already checkpointed.
        state
            .restore_pending_runtime_feedback_at(path.clone())
            .unwrap();
        assert!(!path.exists());
        let called = std::cell::Cell::new(false);
        super::super::checkpoint_autonomous_drain(&mut state, |_| {
            assert_eq!(
                configured(&path).pending_runtime_feedback,
                vec![item.clone()]
            );
            called.set(true);
            Ok(())
        })
        .unwrap();
        assert!(called.get());
        assert_eq!(state.pending_runtime_feedback, vec![item]);
    }

    #[test]
    fn pending_runtime_feedback_drain_refuses_unreadable_stale_or_failed_queue() {
        let directory = tempfile::tempdir().unwrap();
        for case in ["unconfigured", "malformed", "changed", "io-failure"] {
            let path = directory.path().join(format!("{case}.json"));
            let mut state = ConversationState::new(Vec::new(), None);
            match case {
                "malformed" => {
                    std::fs::write(&path, b"unreadable prior pending state").unwrap();
                    assert!(
                        state
                            .restore_pending_runtime_feedback_at(path.clone())
                            .is_err()
                    );
                },
                "changed" => {
                    state
                        .restore_pending_runtime_feedback_at(path.clone())
                        .unwrap();
                    state.enqueue_runtime_feedback(feedback(case));
                    std::fs::write(&path, b"unexpected replacement").unwrap();
                },
                "io-failure" => {
                    state
                        .restore_pending_runtime_feedback_at(path.clone())
                        .unwrap();
                    std::fs::create_dir(&path).unwrap();
                    state.enqueue_runtime_feedback(feedback(case));
                },
                _ => {},
            }
            let before = state.pending_runtime_feedback.clone();
            let called = std::cell::Cell::new(false);
            // Test the production drain ordering with an inert checkpoint
            // callback; even a regression cannot touch a production workspace.
            let result = super::super::checkpoint_autonomous_drain(&mut state, |_| {
                called.set(true);
                Ok(())
            });
            assert!(result.is_err(), "{case}");
            assert!(!called.get(), "{case}");
            assert_eq!(state.pending_runtime_feedback, before, "{case}");
        }
    }

    #[test]
    fn pending_runtime_feedback_descriptor_binds_presence_exact_bytes_and_count() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PENDING_RUNTIME_FEEDBACK_FILE);
        assert_eq!(
            pending_runtime_feedback_descriptor_at(&path).unwrap(),
            serde_json::json!({"present":false})
        );
        assert!(!path.exists());
        let mut state = configured(&path);
        state.enqueue_runtime_feedback(feedback("first"));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            pending_runtime_feedback_descriptor_at(&path).unwrap(),
            serde_json::json!({
                "present":true, "schema":SCHEMA, "sha256":digest(&bytes), "pending_count":1
            })
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let pretty = serde_json::to_vec_pretty(
            &serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        )
        .unwrap();
        crate::lifecycle::atomic_private_write(&path, &pretty).unwrap();
        assert_ne!(
            pending_runtime_feedback_descriptor_at(&path).unwrap()["sha256"],
            digest(&bytes)
        );
        assert_eq!(
            pending_runtime_feedback_descriptor_at(&path).unwrap()["pending_count"],
            1
        );
    }

    #[test]
    fn pending_runtime_feedback_descriptor_refuses_malformed_duplicate_or_nonprivate_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PENDING_RUNTIME_FEEDBACK_FILE);
        let item = feedback("duplicate");
        for bytes in [
            b"{broken".to_vec(),
            serde_json::to_vec(&serde_json::json!({"schema":"future", "pending_runtime_feedback":[]})).unwrap(),
            serde_json::to_vec(&serde_json::json!({"schema":SCHEMA, "pending_runtime_feedback":[item.clone(), item.clone()]})).unwrap(),
        ] {
            crate::lifecycle::atomic_private_write(&path, &bytes).unwrap();
            assert!(pending_runtime_feedback_descriptor_at(&path).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, symlink};
            let mut state = ConversationState::new(Vec::new(), None);
            std::fs::remove_file(&path).unwrap();
            state
                .restore_pending_runtime_feedback_at(path.clone())
                .unwrap();
            state.enqueue_runtime_feedback(item);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(pending_runtime_feedback_descriptor_at(&path).is_err());
            let linked = directory.path().join("linked.json");
            symlink(&path, &linked).unwrap();
            assert!(pending_runtime_feedback_descriptor_at(&linked).is_err());
        }
    }

    #[test]
    fn pending_runtime_feedback_descriptor_refuses_oversize_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PENDING_RUNTIME_FEEDBACK_FILE);
        crate::lifecycle::atomic_private_write(&path, b"{}").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(DESCRIPTOR_LIMIT_BYTES.saturating_add(1))
            .unwrap();
        assert!(
            pending_runtime_feedback_descriptor_at(&path)
                .unwrap_err()
                .to_string()
                .contains("64 MiB")
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            DESCRIPTOR_LIMIT_BYTES.saturating_add(1)
        );
        let before = file.metadata().unwrap();
        let other = directory.path().join("other.json");
        crate::lifecycle::atomic_private_write(&other, b"{}").unwrap();
        assert!(!same_descriptor_file(
            &before,
            &std::fs::metadata(other).unwrap()
        ));
    }
}
