//! Authenticated, append-only history for model-authored candidate operations.
//!
//! Records intentionally retain hashes and bounded counts only. Candidate source,
//! replacement bodies, patches, diffs, prompts, and model response text never enter
//! this ledger.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::util::{canonical_json, read_stable_regular, sha256, unix_seconds};
use crate::{Error, Result};

const RECORD_SCHEMA: &str = "astrid.edge.steward_helper.candidate_edit_record.v1";
const ENVELOPE_SCHEMA: &str = "astrid.edge.steward_helper.candidate_edit_envelope.v1";
const MAX_LEDGER_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 32 * 1024;
const MAX_METADATA_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct EventContext<'a> {
    pub due_nonce: &'a str,
    pub trace_id: &'a str,
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub response_sha256: &'a str,
    pub declaration_sha256: &'a str,
    pub context_provenance_sha256: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedRecord {
    pub envelope: Value,
    pub envelope_sha256: String,
    pub event_id: String,
    pub event_binding_sha256: String,
    pub sequence: u64,
    pub previous_record_sha256: Option<String>,
    pub record_sha256: String,
}

#[derive(Debug)]
struct VerifiedLedger {
    records: Vec<PreparedRecord>,
    head: Option<String>,
}

pub struct CandidateEditLedger<'a> {
    path: PathBuf,
    signer: &'a HmacSigner,
}

impl<'a> CandidateEditLedger<'a> {
    pub fn new(state_root: &Path, signer: &'a HmacSigner) -> Self {
        Self {
            path: state_root.join("candidate-edits.jsonl"),
            signer,
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )] // The complete authenticated event binding is validated and moved into one immutable record.
    pub fn prepare(
        &self,
        operation: &str,
        outcome: &str,
        candidate_id: Option<&str>,
        source_id: Option<&str>,
        base_generation: Option<&str>,
        before_draft_sha256: Option<&str>,
        after_draft_sha256: Option<&str>,
        metadata: Value,
        context: &EventContext<'_>,
    ) -> Result<PreparedRecord> {
        validate_operation(operation)?;
        if !matches!(outcome, "completed" | "rejected") {
            return Err(Error::new("candidate edit outcome is invalid"));
        }
        for (value, label) in [
            (context.due_nonce, "due nonce"),
            (context.trace_id, "trace id"),
            (context.session_id, "session id"),
            (context.turn_id, "turn id"),
        ] {
            crate::util::validate_identifier(value, label)?;
        }
        for (value, label) in [
            (context.response_sha256, "response hash"),
            (context.declaration_sha256, "declaration hash"),
            (context.context_provenance_sha256, "context provenance hash"),
        ] {
            crate::util::validate_hex64(value, label)?;
        }
        for (value, label) in [
            (candidate_id, "candidate id"),
            (base_generation, "base generation"),
        ] {
            if let Some(value) = value {
                crate::util::validate_identifier(value, label)?;
            }
        }
        if let Some(source_id) = source_id {
            let digest = source_id
                .strip_prefix("cpu-edge:")
                .ok_or_else(|| Error::new("candidate edit source identity is invalid"))?;
            crate::util::validate_hex64(digest, "source id")?;
        }
        for (value, label) in [
            (before_draft_sha256, "prior draft hash"),
            (after_draft_sha256, "next draft hash"),
        ] {
            if let Some(value) = value {
                crate::util::validate_hex64(value, label)?;
            }
        }
        validate_metadata(&metadata)?;
        let binding = serde_json::json!({
            "schema": "astrid.edge.steward_helper.candidate_edit_binding.v1",
            "operation": operation,
            "outcome": outcome,
            "candidate_id": candidate_id,
            "source_id": source_id,
            "base_generation": base_generation,
            "before_draft_sha256": before_draft_sha256,
            "after_draft_sha256": after_draft_sha256,
            "due_nonce": context.due_nonce,
            "trace_id": context.trace_id,
            "session_id": context.session_id,
            "turn_id": context.turn_id,
            "response_sha256": context.response_sha256,
            "declaration_sha256": context.declaration_sha256,
            "context_provenance_sha256": context.context_provenance_sha256,
            "metadata": metadata,
            "authority": "model_authored_candidate_operation_audit_not_deployment_authority"
        });
        let binding_sha256 = sha256(&canonical_json(&binding)?);
        let event_id = format!("candidate-edit-{}", &binding_sha256[..24]);
        let verified = self.verify()?;
        if let Some(existing) = verified
            .records
            .iter()
            .find(|record| record.event_id == event_id)
        {
            if existing.event_binding_sha256 != binding_sha256 {
                return Err(Error::new("candidate edit event ID collision"));
            }
            return Ok(existing.clone());
        }
        let sequence = u64::try_from(verified.records.len())
            .map_err(|_| Error::new("candidate edit ledger sequence overflow"))?
            .saturating_add(1);
        let core = serde_json::json!({
            "schema": RECORD_SCHEMA,
            "sequence": sequence,
            "previous_record_sha256": verified.head,
            "event_id": event_id,
            "event_binding_sha256": binding_sha256,
            "recorded_at": unix_seconds(),
            "binding": binding
        });
        let core_bytes = canonical_json(&core)?;
        let record_sha256 = sha256(&core_bytes);
        let envelope = serde_json::json!({
            "schema": ENVELOPE_SCHEMA,
            "core": core,
            "record_sha256": record_sha256,
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": self.signer.key_id,
                "signature": self.signer.sign(&core_bytes)
            }
        });
        let envelope_bytes = canonical_json(&envelope)?;
        if envelope_bytes.len().saturating_add(1) > MAX_LINE_BYTES {
            return Err(Error::new("candidate edit record exceeds its line bound"));
        }
        Ok(PreparedRecord {
            envelope,
            envelope_sha256: sha256(&envelope_bytes),
            event_id,
            event_binding_sha256: binding_sha256,
            sequence,
            previous_record_sha256: verified.head,
            record_sha256,
        })
    }

    pub fn append(&self, prepared: &PreparedRecord) -> Result<()> {
        self.append_with_hook(prepared, || {})
    }

    fn append_with_hook(
        &self,
        prepared: &PreparedRecord,
        before_final_identity_check: impl FnOnce(),
    ) -> Result<()> {
        let envelope_bytes = canonical_json(&prepared.envelope)?;
        if sha256(&envelope_bytes) != prepared.envelope_sha256 {
            return Err(Error::new("prepared candidate edit envelope changed"));
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::new("candidate edit ledger has no parent"))?;
        crate::util::ensure_private_dir(parent)?;
        let before = fs::symlink_metadata(&self.path).ok();
        if before.as_ref().is_some_and(|metadata| {
            !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1
        }) {
            return Err(Error::new(
                "candidate edit ledger is linked or not a regular file",
            ));
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW);
        let mut file = options.open(&self.path)?;
        file.lock_exclusive()?;
        let opened = file.metadata()?;
        validate_opened_ledger(&opened)?;
        if before
            .as_ref()
            .is_some_and(|metadata| file_identity(metadata) != file_identity(&opened))
            || fs::symlink_metadata(&self.path)
                .map(|metadata| file_identity(&metadata) != file_identity(&opened))
                .unwrap_or(true)
        {
            return Err(Error::new(
                "candidate edit ledger identity changed during open",
            ));
        }
        let existing_bytes = read_locked_ledger(&mut file)?;
        let verified = self.verify_bytes(&existing_bytes)?;
        if let Some(existing) = verified
            .records
            .iter()
            .find(|record| record.event_id == prepared.event_id)
        {
            if existing.envelope_sha256 != prepared.envelope_sha256 {
                return Err(Error::new(
                    "candidate edit replay differs from its first record",
                ));
            }
            return Ok(());
        }
        let expected_sequence = u64::try_from(verified.records.len())
            .map_err(|_| Error::new("candidate edit ledger sequence overflow"))?
            .saturating_add(1);
        if prepared.sequence != expected_sequence
            || prepared.previous_record_sha256 != verified.head
            || envelope_bytes.len().saturating_add(1) > MAX_LINE_BYTES
        {
            return Err(Error::new(
                "candidate edit ledger head changed before append",
            ));
        }
        let append_bytes = u64::try_from(envelope_bytes.len().saturating_add(1))
            .map_err(|_| Error::new("candidate edit record byte count overflow"))?;
        if opened.len().saturating_add(append_bytes) > MAX_LEDGER_BYTES {
            return Err(Error::new("candidate edit ledger exceeds its file bound"));
        }
        let mut line = envelope_bytes;
        line.push(b'\n');
        file.write_all(&line)?;
        file.sync_all()?;
        before_final_identity_check();
        let after_metadata = file.metadata()?;
        let path_metadata = fs::symlink_metadata(&self.path)?;
        let final_metadata = file.metadata()?;
        validate_opened_ledger(&after_metadata)?;
        validate_opened_ledger(&path_metadata)?;
        validate_opened_ledger(&final_metadata)?;
        if file_version(&after_metadata) != file_version(&final_metadata)
            || file_version(&path_metadata) != file_version(&final_metadata)
            || final_metadata.len() != opened.len().saturating_add(append_bytes)
        {
            return Err(Error::new(
                "candidate edit ledger path was replaced during append",
            ));
        }
        let after_bytes = read_locked_ledger(&mut file)?;
        let after = self.verify_bytes(&after_bytes)?;
        if after.head.as_deref() != Some(prepared.record_sha256.as_str()) {
            return Err(Error::new(
                "candidate edit append did not become the exact head",
            ));
        }
        File::open(parent)?.sync_all()?;
        Ok(())
    }

    fn verify(&self) -> Result<VerifiedLedger> {
        if !self.path.exists() {
            if self.path.is_symlink() {
                return Err(Error::new("candidate edit ledger is a broken symlink"));
            }
            return Ok(VerifiedLedger {
                records: Vec::new(),
                head: None,
            });
        }
        validate_opened_ledger(&fs::symlink_metadata(&self.path)?)?;
        let bytes = read_stable_regular(&self.path, MAX_LEDGER_BYTES)?;
        self.verify_bytes(&bytes)
    }

    fn verify_bytes(&self, bytes: &[u8]) -> Result<VerifiedLedger> {
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(Error::new("candidate edit ledger has an incomplete tail"));
        }
        let mut records = Vec::new();
        let mut previous: Option<String> = None;
        for (index, line) in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .enumerate()
        {
            if line.len().saturating_add(1) > MAX_LINE_BYTES {
                return Err(Error::new("candidate edit ledger line is oversized"));
            }
            let envelope: Value = serde_json::from_slice(line)?;
            let core = envelope
                .get("core")
                .ok_or_else(|| Error::new("candidate edit envelope has no core"))?;
            let core_bytes = canonical_json(core)?;
            let record_sha256 = sha256(&core_bytes);
            let auth = envelope
                .get("auth")
                .and_then(Value::as_object)
                .ok_or_else(|| Error::new("candidate edit envelope has no authentication"))?;
            let sequence = core
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::new("candidate edit sequence is absent"))?;
            let expected_sequence = u64::try_from(index)
                .map_err(|_| Error::new("candidate edit sequence overflow"))?
                .saturating_add(1);
            let event_id = core
                .get("event_id")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("candidate edit event ID is absent"))?;
            let binding_sha256 = core
                .get("event_binding_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("candidate edit binding hash is absent"))?;
            crate::util::validate_identifier(event_id, "candidate edit event id")?;
            crate::util::validate_hex64(binding_sha256, "candidate edit binding hash")?;
            if envelope.get("schema").and_then(Value::as_str) != Some(ENVELOPE_SCHEMA)
                || core.get("schema").and_then(Value::as_str) != Some(RECORD_SCHEMA)
                || sequence != expected_sequence
                || core.get("previous_record_sha256")
                    != Some(&previous.clone().map_or(Value::Null, Value::String))
                || envelope.get("record_sha256").and_then(Value::as_str)
                    != Some(record_sha256.as_str())
                || auth.get("algorithm").and_then(Value::as_str) != Some("hmac-sha256")
                || auth.get("key_id").and_then(Value::as_str) != Some(self.signer.key_id.as_str())
                || !auth
                    .get("signature")
                    .and_then(Value::as_str)
                    .is_some_and(|signature| self.signer.verify(&core_bytes, signature))
                || records
                    .iter()
                    .any(|record: &PreparedRecord| record.event_id == event_id)
            {
                return Err(Error::new(
                    "candidate edit ledger authentication or chain failed",
                ));
            }
            let binding = core
                .get("binding")
                .ok_or_else(|| Error::new("candidate edit binding is absent"))?;
            if sha256(&canonical_json(binding)?) != binding_sha256 {
                return Err(Error::new("candidate edit binding digest failed"));
            }
            let event_id = event_id.to_owned();
            let binding_sha256 = binding_sha256.to_owned();
            let envelope_sha256 = sha256(&canonical_json(&envelope)?);
            records.push(PreparedRecord {
                envelope,
                envelope_sha256,
                event_id,
                event_binding_sha256: binding_sha256,
                sequence,
                previous_record_sha256: previous.clone(),
                record_sha256: record_sha256.clone(),
            });
            previous = Some(record_sha256);
        }
        Ok(VerifiedLedger {
            records,
            head: previous,
        })
    }
}

#[allow(clippy::verbose_bit_mask)] // The exact owner-only permission mask is audit-significant.
fn validate_opened_ledger(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAX_LEDGER_BYTES
    {
        return Err(Error::new(
            "candidate edit ledger must be owner-only, regular, single-linked, and bounded",
        ));
    }
    Ok(())
}

fn file_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn file_version(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn read_locked_ledger(file: &mut File) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(MAX_LEDGER_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LEDGER_BYTES {
        return Err(Error::new("candidate edit ledger exceeds its file bound"));
    }
    Ok(bytes)
}

fn validate_operation(operation: &str) -> Result<()> {
    if !matches!(
        operation,
        "begin_candidate"
            | "apply_candidate_patch"
            | "format_candidate"
            | "inspect_candidate"
            | "submit_candidate"
            | "submit_candidate_attested"
            | "abandon_candidate"
            | "candidate_operation_rejected"
    ) {
        return Err(Error::new("candidate edit operation is unsupported"));
    }
    Ok(())
}

fn validate_metadata(value: &Value) -> Result<()> {
    let bytes = canonical_json(value)?;
    if bytes.len() > MAX_METADATA_BYTES || contains_forbidden_key(value) {
        return Err(Error::new(
            "candidate edit metadata contains source/diff content or exceeds its bound",
        ));
    }
    Ok(())
}

fn contains_forbidden_key(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "content" | "source_body" | "response" | "prompt" | "patch" | "diff"
            ) || contains_forbidden_key(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_key),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::{CandidateEditLedger, EventContext};
    use crate::attestation::HmacSigner;

    fn context<'a>() -> EventContext<'a> {
        EventContext {
            due_nonce: "due-10000",
            trace_id: "11111111-1111-4111-8111-111111111111",
            session_id: "session-one",
            turn_id: "22222222-2222-4222-8222-222222222222",
            response_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            declaration_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            context_provenance_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        }
    }

    #[test]
    fn exact_event_replay_is_idempotent_and_changed_replay_fails() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let ledger = CandidateEditLedger::new(temporary.path(), &signer);
        let event = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                Some("candidate-one"),
                Some(&format!("cpu-edge:{}", "c".repeat(64))),
                Some("generation-one"),
                Some(&"d".repeat(64)),
                Some(&"d".repeat(64)),
                serde_json::json!({"files": 1, "changed_lines": 2}),
                &context(),
            )
            .unwrap();
        ledger.append(&event).unwrap();
        ledger.append(&event).unwrap();
        let changed = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                Some("candidate-one"),
                Some(&format!("cpu-edge:{}", "c".repeat(64))),
                Some("generation-one"),
                Some(&"d".repeat(64)),
                Some(&"d".repeat(64)),
                serde_json::json!({"files": 2, "changed_lines": 2}),
                &context(),
            )
            .unwrap();
        assert_ne!(event.event_id, changed.event_id);
    }

    #[test]
    fn linked_ledgers_and_source_bodies_are_rejected() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let ledger = CandidateEditLedger::new(temporary.path(), &signer);
        assert!(
            ledger
                .prepare(
                    "inspect_candidate",
                    "completed",
                    None,
                    None,
                    None,
                    None,
                    None,
                    serde_json::json!({"content": "not allowed"}),
                    &context(),
                )
                .is_err()
        );
        let outside = temporary.path().join("outside");
        fs::write(&outside, b"not a ledger\n").unwrap();
        symlink(&outside, temporary.path().join("candidate-edits.jsonl")).unwrap();
        assert!(
            ledger
                .prepare(
                    "inspect_candidate",
                    "completed",
                    None,
                    None,
                    None,
                    None,
                    None,
                    serde_json::json!({}),
                    &context(),
                )
                .is_err()
        );

        let hardlink_root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let hardlink_key = hardlink_root.path().join("key");
        fs::write(&hardlink_key, [b'k'; 32]).unwrap();
        let hardlink_signer = HmacSigner::from_file(&hardlink_key).unwrap();
        let outside = hardlink_root.path().join("outside");
        fs::write(&outside, b"").unwrap();
        fs::hard_link(&outside, hardlink_root.path().join("candidate-edits.jsonl")).unwrap();
        let hardlinked = CandidateEditLedger::new(hardlink_root.path(), &hardlink_signer);
        assert!(
            hardlinked
                .prepare(
                    "inspect_candidate",
                    "completed",
                    None,
                    None,
                    None,
                    None,
                    None,
                    serde_json::json!({}),
                    &context(),
                )
                .is_err()
        );
    }

    #[test]
    fn candidate_ledger_detects_replacement_after_locked_append() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let ledger = CandidateEditLedger::new(temporary.path(), &signer);
        let event = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                None,
                None,
                None,
                None,
                None,
                serde_json::json!({"files": 0}),
                &context(),
            )
            .unwrap();
        let path = temporary.path().join("candidate-edits.jsonl");
        let replacement = temporary.path().join("replacement");
        fs::write(&replacement, b"").unwrap();
        let result = ledger.append_with_hook(&event, || {
            fs::rename(&replacement, &path).unwrap();
        });
        assert!(result.is_err());
        assert!(fs::read(path).unwrap().is_empty());
    }

    #[test]
    fn concurrent_prepared_heads_serialize_without_chain_corruption() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let ledger = CandidateEditLedger::new(temporary.path(), &signer);
        let first = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                None,
                None,
                None,
                None,
                None,
                serde_json::json!({"ordinal": 1}),
                &context(),
            )
            .unwrap();
        ledger.append(&first).unwrap();
        let second = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                None,
                None,
                None,
                None,
                None,
                serde_json::json!({"ordinal": 2}),
                &context(),
            )
            .unwrap();
        let competing = ledger
            .prepare(
                "inspect_candidate",
                "completed",
                None,
                None,
                None,
                None,
                None,
                serde_json::json!({"ordinal": 3}),
                &context(),
            )
            .unwrap();
        let outcomes = std::thread::scope(|scope| {
            let left = scope.spawn(|| ledger.append(&second));
            let right = scope.spawn(|| ledger.append(&competing));
            [left.join().unwrap(), right.join().unwrap()]
        });
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(ledger.verify().unwrap().records.len(), 2);
    }
}
