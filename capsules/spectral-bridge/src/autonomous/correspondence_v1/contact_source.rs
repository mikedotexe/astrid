use std::path::{Path, PathBuf};

use super::{
    canonical_legacy_source_path, legacy_kind_for_path, legacy_message_id, parse_envelope_text,
    sha256_hex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InboxPeerMessage {
    pub message_id: String,
    pub thread_id: String,
    pub persistence_id: Option<String>,
    pub from_being: String,
    pub file_path: PathBuf,
}

/// Bounded local identity for an inbox source that the existing
/// correspondence routes already know how to interpret. This is provenance
/// validation within the shared workspace, not cryptographic sender
/// authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InboxContactSourceIdentityV1 {
    pub principal: String,
    pub source_ref_id: String,
    pub thread_id: Option<String>,
    pub source_body_sha256: String,
    pub provenance_basis: &'static str,
}

#[must_use]
pub(crate) fn contact_source_identity_for_inbox_file(
    path: &Path,
    content: &str,
) -> Option<InboxContactSourceIdentityV1> {
    if let Some(envelope) = parse_envelope_text(content)
        && envelope.to_being == "astrid"
        && !envelope.from_being.trim().is_empty()
    {
        return Some(InboxContactSourceIdentityV1 {
            principal: envelope.from_being,
            source_ref_id: envelope.message_id,
            thread_id: Some(envelope.thread_id),
            source_body_sha256: sha256_hex(&envelope.body),
            provenance_basis: "correspondence_v1_envelope",
        });
    }

    let name = path.file_name()?.to_str()?;
    if name.starts_with("mike_query_") || name.starts_with("mike_feedback_") {
        return Some(InboxContactSourceIdentityV1 {
            principal: "mike".to_string(),
            source_ref_id: name.to_string(),
            thread_id: None,
            source_body_sha256: sha256_hex(content),
            provenance_basis: "sanctioned_owner_note_v1",
        });
    }
    if name.starts_with("steward_") || name.starts_with("from_steward_") {
        return Some(InboxContactSourceIdentityV1 {
            principal: "steward".to_string(),
            source_ref_id: name.to_string(),
            thread_id: None,
            source_body_sha256: sha256_hex(content),
            provenance_basis: "sanctioned_steward_note_v1",
        });
    }

    let (from_being, to_being, _, _) = legacy_kind_for_path(path, content)?;
    if from_being != "minime" || to_being != "astrid" {
        return None;
    }
    let canonical_path = canonical_legacy_source_path(path);
    let source_body_sha256 = sha256_hex(content);
    Some(InboxContactSourceIdentityV1 {
        principal: "minime".to_string(),
        source_ref_id: legacy_message_id(
            from_being,
            to_being,
            &canonical_path,
            &source_body_sha256,
        ),
        thread_id: None,
        source_body_sha256,
        provenance_basis: "legacy_correspondence_mapper_v1",
    })
}
