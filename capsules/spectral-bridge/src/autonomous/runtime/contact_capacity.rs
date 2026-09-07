use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ops::Deref;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::correspondence_v1::InboxContactSourceIdentityV1;
use crate::signal_spine::{
    SignalJourneyOriginV1, SignalProcessIdentityV1, canonical_sha256, signal_monotonic_ns_v1,
    signal_process_identity_v1, signal_unix_ms_v1,
};

const CONTACT_TRACE_SCHEMA_V1: &str = "contact_input_receipt_v1";
const INGRESS_GAP_SCHEMA_V1: &str = "contact_ingress_gap_v1";

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn system_time_unix_ms(value: SystemTime) -> u64 {
    value.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        duration.as_millis().try_into().unwrap_or(u64::MAX)
    })
}

fn integrity_sha256<T: Serialize>(value: &T) -> String {
    let mut document = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Value::Object(fields) = &mut document {
        fields.remove("receipt_sha256");
    }
    canonical_sha256(&document)
}

fn bounded_error_kind(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;

    match error.kind() {
        ErrorKind::AlreadyExists => "already_exists",
        ErrorKind::NotFound => "not_found",
        ErrorKind::PermissionDenied => "permission_denied",
        ErrorKind::InvalidInput => "invalid_input",
        ErrorKind::InvalidData => "invalid_data",
        ErrorKind::StorageFull => "storage_full",
        ErrorKind::ReadOnlyFilesystem => "read_only_filesystem",
        _ => "io_error",
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InboxSourceMaterialV1 {
    source_ref_fallback: String,
    source_bytes_sha256: String,
    identity: Option<InboxContactSourceIdentityV1>,
}

impl InboxSourceMaterialV1 {
    pub(crate) fn new(
        source_ref_fallback: String,
        source_bytes: &[u8],
        identity: Option<InboxContactSourceIdentityV1>,
    ) -> Self {
        Self {
            source_ref_fallback,
            source_bytes_sha256: sha256_bytes(source_bytes),
            identity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ContactSourceRefV1 {
    source_ref_id_sha256: String,
    source_bytes_sha256: String,
    source_body_sha256: Option<String>,
    principal_identity_sha256: Option<String>,
    thread_id_sha256: Option<String>,
    provenance_state: &'static str,
    provenance_basis: &'static str,
    local_provenance_validated: bool,
    cryptographic_sender_authenticated: bool,
}

impl From<InboxSourceMaterialV1> for ContactSourceRefV1 {
    fn from(source: InboxSourceMaterialV1) -> Self {
        match source.identity {
            Some(identity) => Self {
                source_ref_id_sha256: sha256_bytes(identity.source_ref_id.as_bytes()),
                source_bytes_sha256: source.source_bytes_sha256,
                source_body_sha256: Some(identity.source_body_sha256),
                principal_identity_sha256: Some(sha256_bytes(identity.principal.as_bytes())),
                thread_id_sha256: identity
                    .thread_id
                    .as_deref()
                    .map(|thread_id| sha256_bytes(thread_id.as_bytes())),
                provenance_state: "provenance_backed",
                provenance_basis: identity.provenance_basis,
                local_provenance_validated: true,
                cryptographic_sender_authenticated: false,
            },
            None => Self {
                source_ref_id_sha256: sha256_bytes(source.source_ref_fallback.as_bytes()),
                source_bytes_sha256: source.source_bytes_sha256,
                source_body_sha256: None,
                principal_identity_sha256: None,
                thread_id_sha256: None,
                provenance_state: "unverified",
                provenance_basis: "unverified_inbox_source",
                local_provenance_validated: false,
                cryptographic_sender_authenticated: false,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ContactInputReceiptV1<'a> {
    schema: &'static str,
    schema_version: u8,
    contact_id: &'a str,
    journey_id: &'a str,
    cutoff_unix_ms: u64,
    clock_scope_id: &'a str,
    input_identity_sha256: &'a str,
    principal_identity_sha256: &'a str,
    process_identity_sha256: &'a str,
    received_at_unix_ms: u64,
    received_monotonic_ns: u64,
    admitted_context_sha256: &'a str,
    admission_outcome: &'static str,
    admission_basis: &'static str,
    provenance_state: &'static str,
    source_count: usize,
    source_refs: &'a [ContactSourceRefV1],
    local_provenance_validation_only: bool,
    cryptographic_sender_authentication: bool,
    raw_input_included: bool,
    live_control_authority: bool,
    receipt_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct ContactIngressGapV1<'a> {
    schema: &'static str,
    schema_version: u8,
    ingress_gap_id: &'a str,
    journey_id: &'a str,
    attempted_contact_id: Option<&'a str>,
    reason: &'static str,
    capture_error_kind: Option<&'static str>,
    cutoff_unix_ms: u64,
    clock_scope_id: &'a str,
    input_identity_sha256: &'a str,
    process_identity_sha256: &'a str,
    received_at_unix_ms: u64,
    received_monotonic_ns: u64,
    admitted_context_sha256: &'a str,
    admission_outcome: &'static str,
    provenance_state: &'static str,
    source_count: usize,
    source_refs: &'a [ContactSourceRefV1],
    local_provenance_validation_only: bool,
    cryptographic_sender_authentication: bool,
    raw_input_included: bool,
    live_control_authority: bool,
    receipt_sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct InboxReadBatchV1 {
    prompt_text: String,
    cutoff_unix_ms: u64,
    received_at_unix_ms: u64,
    received_monotonic_ns: u64,
    input_identity_sha256: String,
    admitted_context_sha256: String,
    process_identity_sha256: String,
    source_refs: Vec<ContactSourceRefV1>,
    provenance_state: &'static str,
    contact_id: Option<String>,
    ingress_gap_id: Option<String>,
    journey_id: String,
    journey_origin: SignalJourneyOriginV1,
}

impl InboxReadBatchV1 {
    pub(crate) fn capture(
        prompt_text: String,
        cutoff: SystemTime,
        sources: Vec<InboxSourceMaterialV1>,
        trace_root: &Path,
    ) -> Self {
        let batch = Self::capture_with_clock(
            prompt_text,
            cutoff,
            sources,
            trace_root,
            signal_unix_ms_v1(),
            signal_monotonic_ns_v1(),
            signal_process_identity_v1(),
        );
        batch.log_evidence_state();
        batch
    }

    fn capture_with_clock(
        prompt_text: String,
        cutoff: SystemTime,
        sources: Vec<InboxSourceMaterialV1>,
        trace_root: &Path,
        received_at_unix_ms: u64,
        received_monotonic_ns: u64,
        process_identity: SignalProcessIdentityV1,
    ) -> Self {
        let source_refs = sources
            .into_iter()
            .map(ContactSourceRefV1::from)
            .collect::<Vec<_>>();
        let all_provenance_backed = source_refs
            .iter()
            .all(|source| source.local_provenance_validated);
        let provenance_state = if all_provenance_backed {
            "provenance_backed"
        } else {
            "mixed_or_unverified"
        };
        let cutoff_unix_ms = system_time_unix_ms(cutoff);
        let input_identity_sha256 =
            canonical_sha256(&serde_json::to_value(&source_refs).unwrap_or(Value::Null));
        let admitted_context_sha256 = sha256_bytes(prompt_text.as_bytes());
        let process_identity_sha256 = process_identity.canonical_sha256();
        let principal_identity_sha256 = {
            let principals = source_refs
                .iter()
                .filter_map(|source| source.principal_identity_sha256.as_deref())
                .collect::<BTreeSet<_>>();
            canonical_sha256(&json!(principals))
        };
        let event_seed = json!({
            "schema": CONTACT_TRACE_SCHEMA_V1,
            "cutoff_unix_ms": cutoff_unix_ms,
            "clock_scope_id": process_identity.clock_scope_id(),
            "input_identity_sha256": input_identity_sha256,
            "admitted_context_sha256": admitted_context_sha256,
            "received_at_unix_ms": received_at_unix_ms,
            "received_monotonic_ns": received_monotonic_ns,
        });
        let event_sha256 = canonical_sha256(&event_seed);
        let contact_id = format!("contact_{}", &event_sha256[..24]);
        let journey_id = format!(
            "journey_{}",
            &canonical_sha256(&json!({
                "event_sha256": event_sha256,
                "role": "contact_response_journey",
            }))[..24]
        );

        if all_provenance_backed {
            let mut receipt = ContactInputReceiptV1 {
                schema: CONTACT_TRACE_SCHEMA_V1,
                schema_version: 1,
                contact_id: &contact_id,
                journey_id: &journey_id,
                cutoff_unix_ms,
                clock_scope_id: process_identity.clock_scope_id(),
                input_identity_sha256: &input_identity_sha256,
                principal_identity_sha256: &principal_identity_sha256,
                process_identity_sha256: &process_identity_sha256,
                received_at_unix_ms,
                received_monotonic_ns,
                admitted_context_sha256: &admitted_context_sha256,
                admission_outcome: "admitted",
                admission_basis: "prompt_context",
                provenance_state,
                source_count: source_refs.len(),
                source_refs: &source_refs,
                local_provenance_validation_only: true,
                cryptographic_sender_authentication: false,
                raw_input_included: false,
                live_control_authority: false,
                receipt_sha256: String::new(),
            };
            receipt.receipt_sha256 = integrity_sha256(&receipt);
            let receipt_sha256 = receipt.receipt_sha256.clone();
            let path = trace_root
                .join("contact_receipts")
                .join(format!("{contact_id}.json"));
            match write_owner_only_new(&path, &receipt) {
                Ok(()) => Self {
                    prompt_text,
                    cutoff_unix_ms,
                    received_at_unix_ms,
                    received_monotonic_ns,
                    input_identity_sha256,
                    admitted_context_sha256,
                    process_identity_sha256,
                    source_refs,
                    provenance_state,
                    contact_id: Some(contact_id.clone()),
                    ingress_gap_id: None,
                    journey_id,
                    journey_origin: SignalJourneyOriginV1::contact(contact_id, receipt_sha256),
                },
                Err(error) => Self::capture_gap(
                    prompt_text,
                    cutoff_unix_ms,
                    received_at_unix_ms,
                    received_monotonic_ns,
                    input_identity_sha256,
                    admitted_context_sha256,
                    process_identity_sha256,
                    source_refs,
                    provenance_state,
                    journey_id,
                    Some(contact_id),
                    "contact_capture_failed",
                    Some(bounded_error_kind(&error)),
                    process_identity.clock_scope_id(),
                    trace_root,
                ),
            }
        } else {
            Self::capture_gap(
                prompt_text,
                cutoff_unix_ms,
                received_at_unix_ms,
                received_monotonic_ns,
                input_identity_sha256,
                admitted_context_sha256,
                process_identity_sha256,
                source_refs,
                provenance_state,
                journey_id,
                None,
                "mixed_or_unverified_source",
                None,
                process_identity.clock_scope_id(),
                trace_root,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_gap(
        prompt_text: String,
        cutoff_unix_ms: u64,
        received_at_unix_ms: u64,
        received_monotonic_ns: u64,
        input_identity_sha256: String,
        admitted_context_sha256: String,
        process_identity_sha256: String,
        source_refs: Vec<ContactSourceRefV1>,
        provenance_state: &'static str,
        journey_id: String,
        attempted_contact_id: Option<String>,
        reason: &'static str,
        capture_error_kind: Option<&'static str>,
        clock_scope_id: &str,
        trace_root: &Path,
    ) -> Self {
        let gap_seed = json!({
            "journey_id": journey_id,
            "attempted_contact_id": attempted_contact_id,
            "reason": reason,
            "capture_error_kind": capture_error_kind,
            "input_identity_sha256": input_identity_sha256,
        });
        let gap_id = format!("ingress_gap_{}", &canonical_sha256(&gap_seed)[..24]);
        let mut gap = ContactIngressGapV1 {
            schema: INGRESS_GAP_SCHEMA_V1,
            schema_version: 1,
            ingress_gap_id: &gap_id,
            journey_id: &journey_id,
            attempted_contact_id: attempted_contact_id.as_deref(),
            reason,
            capture_error_kind,
            cutoff_unix_ms,
            clock_scope_id,
            input_identity_sha256: &input_identity_sha256,
            process_identity_sha256: &process_identity_sha256,
            received_at_unix_ms,
            received_monotonic_ns,
            admitted_context_sha256: &admitted_context_sha256,
            admission_outcome: "admitted",
            provenance_state,
            source_count: source_refs.len(),
            source_refs: &source_refs,
            local_provenance_validation_only: true,
            cryptographic_sender_authentication: false,
            raw_input_included: false,
            live_control_authority: false,
            receipt_sha256: String::new(),
        };
        gap.receipt_sha256 = integrity_sha256(&gap);
        let gap_sha256 = gap.receipt_sha256.clone();
        let path = trace_root
            .join("ingress_gaps")
            .join(format!("{gap_id}.json"));
        let _ = write_owner_only_new(&path, &gap);
        Self {
            prompt_text,
            cutoff_unix_ms,
            received_at_unix_ms,
            received_monotonic_ns,
            input_identity_sha256,
            admitted_context_sha256,
            process_identity_sha256,
            source_refs,
            provenance_state,
            contact_id: None,
            ingress_gap_id: Some(gap_id.clone()),
            journey_id,
            journey_origin: SignalJourneyOriginV1::ingress_gap(gap_id, reason, Some(gap_sha256)),
        }
    }

    #[must_use]
    pub(crate) fn signal_reservation(&self) -> (String, SignalJourneyOriginV1) {
        (self.journey_id.clone(), self.journey_origin.clone())
    }

    fn log_evidence_state(&self) {
        tracing::info!(
            cutoff_unix_ms = self.cutoff_unix_ms,
            received_at_unix_ms = self.received_at_unix_ms,
            received_monotonic_ns = self.received_monotonic_ns,
            input_identity_sha256 = %self.input_identity_sha256,
            admitted_context_sha256 = %self.admitted_context_sha256,
            process_identity_sha256 = %self.process_identity_sha256,
            source_count = self.source_refs.len(),
            provenance_state = self.provenance_state,
            contact_id = self.contact_id.as_deref().unwrap_or("none"),
            ingress_gap_id = self.ingress_gap_id.as_deref().unwrap_or("none"),
            raw_input_included = false,
            live_control_authority = false,
            "contact input batch captured"
        );
    }

    #[must_use]
    pub(crate) fn prompt_text(&self) -> &str {
        &self.prompt_text
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn journey_id(&self) -> &str {
        &self.journey_id
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn contact_id(&self) -> Option<&str> {
        self.contact_id.as_deref()
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn ingress_gap_id(&self) -> Option<&str> {
        self.ingress_gap_id.as_deref()
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn provenance_state(&self) -> &str {
        self.provenance_state
    }
}

impl Deref for InboxReadBatchV1 {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.prompt_text()
    }
}

impl fmt::Display for InboxReadBatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.prompt_text())
    }
}

fn write_owner_only_new<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(&bytes)?;
    file.flush()
}

#[must_use]
#[cfg(test)]
pub(crate) fn trace_root_for_inbox(inbox_dir: &Path) -> PathBuf {
    inbox_dir.join(".contact_capacity_trace_v1")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::signal_spine::SignalJourneyOriginKindV1;

    fn backed_source(raw: &[u8]) -> InboxSourceMaterialV1 {
        InboxSourceMaterialV1::new(
            "mike_feedback_private_name.txt".to_string(),
            raw,
            Some(InboxContactSourceIdentityV1 {
                principal: "mike".to_string(),
                source_ref_id: "mike_feedback_private_name.txt".to_string(),
                thread_id: None,
                source_body_sha256: sha256_bytes(raw),
                provenance_basis: "sanctioned_owner_note_v1",
            }),
        )
    }

    #[test]
    fn correspondence_classifier_reuses_only_sanctioned_local_routes() {
        let root = tempdir().unwrap();
        let envelope_path = root.path().join("from_minime_correspondence_msg_1.txt");
        let envelope = "=== CORRESPONDENCE V1 ===\n\
                        Message-Id: msg_1\n\
                        Thread-Id: thread_1\n\
                        From: minime\n\
                        To: astrid\n\n\
                        bounded body";
        let identity = super::super::correspondence_v1::contact_source_identity_for_inbox_file(
            &envelope_path,
            envelope,
        )
        .unwrap();
        assert_eq!(identity.principal, "minime");
        assert_eq!(identity.provenance_basis, "correspondence_v1_envelope");

        let legacy = super::super::correspondence_v1::contact_source_identity_for_inbox_file(
            &root.path().join("from_minime_123.txt"),
            "legacy reply",
        )
        .unwrap();
        assert_eq!(legacy.provenance_basis, "legacy_correspondence_mapper_v1");

        let owner = super::super::correspondence_v1::contact_source_identity_for_inbox_file(
            &root.path().join("mike_feedback_bearing_123.txt"),
            "owner note",
        )
        .unwrap();
        assert_eq!(owner.principal, "mike");
        assert_eq!(owner.provenance_basis, "sanctioned_owner_note_v1");

        assert!(
            super::super::correspondence_v1::contact_source_identity_for_inbox_file(
                &root.path().join("unknown.txt"),
                "unverified",
            )
            .is_none()
        );
    }

    #[test]
    fn proven_batch_writes_hash_only_contact_receipt_and_reserves_journey() {
        let root = tempdir().unwrap();
        let batch = InboxReadBatchV1::capture_with_clock(
            "private prompt prose".to_string(),
            UNIX_EPOCH + std::time::Duration::from_secs(10),
            vec![backed_source(b"private prompt prose")],
            root.path(),
            20_000,
            30_000,
            signal_process_identity_v1(),
        );
        assert_eq!(batch.provenance_state(), "provenance_backed");
        assert!(batch.contact_id().is_some());
        assert!(batch.ingress_gap_id().is_none());
        assert!(batch.journey_id().starts_with("journey_"));
        assert_eq!(
            batch.journey_origin.kind(),
            SignalJourneyOriginKindV1::Contact
        );
        let receipt = fs::read_to_string(
            root.path()
                .join("contact_receipts")
                .join(format!("{}.json", batch.contact_id().unwrap())),
        )
        .unwrap();
        assert!(!receipt.contains("private prompt prose"));
        assert!(!receipt.contains("mike_feedback_private_name.txt"));
        assert!(receipt.contains("local_provenance_validation_only"));
        assert!(receipt.contains("\"cryptographic_sender_authentication\": false"));
    }

    #[test]
    fn mixed_batch_preserves_prompt_but_emits_only_an_ingress_gap() {
        let root = tempdir().unwrap();
        let batch = InboxReadBatchV1::capture_with_clock(
            "same prompt".to_string(),
            UNIX_EPOCH + std::time::Duration::from_secs(10),
            vec![
                backed_source(b"known"),
                InboxSourceMaterialV1::new("unknown.txt".to_string(), b"unknown", None),
            ],
            root.path(),
            20_000,
            30_000,
            signal_process_identity_v1(),
        );
        assert_eq!(batch.prompt_text(), "same prompt");
        assert!(batch.contact_id().is_none());
        assert!(batch.ingress_gap_id().is_some());
        assert_eq!(
            batch.journey_origin.kind(),
            SignalJourneyOriginKindV1::IngressGap
        );
        assert!(!root.path().join("contact_receipts").exists());
    }

    #[test]
    fn colliding_contact_receipt_is_never_overwritten() {
        let root = tempdir().unwrap();
        let capture = || {
            InboxReadBatchV1::capture_with_clock(
                "same prompt".to_string(),
                UNIX_EPOCH + std::time::Duration::from_secs(10),
                vec![backed_source(b"same prompt")],
                root.path(),
                20_000,
                30_000,
                signal_process_identity_v1(),
            )
        };
        let first = capture();
        let path = root
            .path()
            .join("contact_receipts")
            .join(format!("{}.json", first.contact_id().unwrap()));
        let original = fs::read(&path).unwrap();
        let second = capture();
        assert!(second.contact_id().is_none());
        assert!(second.ingress_gap_id().is_some());
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn capture_failure_keeps_dialogue_available_and_marks_the_gap() {
        let root = tempdir().unwrap();
        let blocked_root = root.path().join("not_a_directory");
        fs::write(&blocked_root, b"occupied").unwrap();
        let batch = InboxReadBatchV1::capture_with_clock(
            "dialogue remains available".to_string(),
            UNIX_EPOCH + std::time::Duration::from_secs(10),
            vec![backed_source(b"dialogue remains available")],
            &blocked_root,
            20_000,
            30_000,
            signal_process_identity_v1(),
        );
        assert_eq!(batch.prompt_text(), "dialogue remains available");
        assert!(batch.contact_id().is_none());
        assert!(batch.ingress_gap_id().is_some());
    }
}
