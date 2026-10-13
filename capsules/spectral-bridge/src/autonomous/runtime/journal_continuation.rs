//! Optional public-journal continuation, reserved by host lineage before generation.
//! Registration supplies provenance only. A separate authored `WRITE FROM_JOURNAL`
//! choice is the only operation that may seed a private draft.
use anyhow::{Context as _, Result, ensure};
use astrid_source_study::{
    Catalog, JournalByteSpan, JournalPublish, JournalReceipt, JournalRegistration, JournalReserve,
    JournalSourceKind, Reader,
};
use sha2::{Digest as _, Sha256};
use std::path::PathBuf;

pub(super) struct ReservedJournal {
    reader: Reader,
    operation_id: String,
    entry_id: String,
    mode: String,
    hint: String,
}

/// Only returned after the complete document has been successfully saved.
pub(super) struct SavedJournal {
    source_path: PathBuf,
    file_sha256: String,
    response_sha256: String,
    response_span: JournalByteSpan,
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn eligible(mode: &str) -> bool {
    matches!(mode, "dialogue_live" | "daydream" | "aspiration")
}

pub(super) fn reserve_configured(lineage: &str, mode: &str) -> Option<ReservedJournal> {
    if !eligible(mode) {
        return None;
    }
    let reserve = || -> Result<_> {
        let paths = crate::paths::bridge_paths();
        let reader = Reader::new(
            Catalog::installation(paths.astrid_root(), paths.minime_root())?,
            paths
                .bridge_workspace()
                .join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(paths.bridge_workspace().to_path_buf(), "astrid");
        ReservedJournal::reserve(reader, lineage, mode)
    };
    match reserve() {
        Ok(reservation) => Some(reservation),
        Err(error) => {
            tracing::warn!(%error, "public journal continuation unavailable; ordinary generation remains optional");
            None
        },
    }
}

impl ReservedJournal {
    fn reserve(reader: Reader, lineage: &str, mode: &str) -> Result<Self> {
        ensure!(
            eligible(mode),
            "only ordinary authored public journal modes may reserve continuation"
        );
        ensure!(
            !lineage.is_empty() && lineage.len() <= 192 && !lineage.chars().any(char::is_control),
            "bounded host exchange lineage required"
        );
        let operation_id = format!(
            "astrid-public-journal-v1:{}",
            digest(serde_json::to_vec(&(lineage, mode))?)
        );
        let request = JournalRegistration::Reserve(JournalReserve {
            owner: "astrid".into(),
            operation_id: operation_id.clone(),
            source_kind: JournalSourceKind::AuthoredPublicJournal,
        });
        let receipt = register(&reader, &operation_id, "reserve", &request)?;
        ensure!(
            receipt.entry_id.len() == 64 && receipt.entry_id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid reserved public journal identity"
        );
        let hint = format!(
            "Optional private continuation of the public journal you are writing now: \
             choose `NEXT: WRITE FROM_JOURNAL {}`. This exact entry is reserved for this response and \
             becomes available only after its public file is saved and verified. Choosing it \
             copies visible authored prose into a new private draft; it does not alter the \
             public original or authorize sharing. You may instead stop or choose another action.",
            receipt.entry_id,
        );
        Ok(Self {
            reader,
            operation_id,
            entry_id: receipt.entry_id,
            mode: mode.into(),
            hint,
        })
    }

    pub(super) fn hint(&self) -> &str {
        &self.hint
    }

    fn publish(&self, mode: &str, response: &str, saved: SavedJournal) -> Result<()> {
        ensure!(
            eligible(mode) && self.mode == mode,
            "journal generation mode changed; no publication"
        );
        ensure!(
            digest(response) == saved.response_sha256,
            "saved journal response changed; no publication"
        );
        let request = JournalRegistration::Publish(JournalPublish {
            owner: "astrid".into(),
            operation_id: self.operation_id.clone(),
            entry_id: self.entry_id.clone(),
            source_path: saved.source_path,
            file_sha256: saved.file_sha256,
            response_sha256: saved.response_sha256,
            authored_response: response.into(),
            response_span: Some(saved.response_span),
        });
        let receipt = register(&self.reader, &self.operation_id, "publish", &request)?;
        ensure!(
            receipt.published && receipt.entry_id == self.entry_id,
            "public journal publication not confirmed"
        );
        Ok(())
    }
}

fn register(
    reader: &Reader,
    operation: &str,
    phase: &str,
    request: &JournalRegistration,
) -> Result<JournalReceipt> {
    let request_id = format!("journal-{phase}-{}", digest(operation));
    let revision = reader.preparation_revision()?;
    let output = reader.prepare_journal_once(&request_id, &revision, request)?;
    ensure!(
        !output.generation_requested,
        "journal registration must not request generation"
    );
    serde_json::from_str(&output.text).context("invalid native journal receipt")
}

impl SavedJournal {
    /// The renderer appends one newline to the exact response. Any normalization
    /// means there is no exact response attestation, not permission to substitute.
    pub(super) fn exact(source_path: PathBuf, document: &str, response: &str) -> Option<Self> {
        let end = document.len().checked_sub(1)?;
        let start = end.checked_sub(response.len())?;
        if response.trim().is_empty()
            || !document.ends_with('\n')
            || document.get(start..end)? != response
        {
            return None;
        }
        Some(Self {
            source_path,
            file_sha256: digest(document),
            response_sha256: digest(response),
            response_span: JournalByteSpan { start, end },
        })
    }
}

pub(super) fn publish_saved(
    reservation: Option<&ReservedJournal>,
    mode: &str,
    response: &str,
    saved: Option<SavedJournal>,
) {
    let (Some(reservation), Some(saved)) = (reservation, saved) else {
        return;
    };
    if let Err(error) = reservation.publish(mode, response, saved) {
        tracing::warn!(%error, "public journal saved but continuation registration unavailable; no private transfer");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, fs, path::Path};

    fn reader(root: &Path) -> Reader {
        Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), root.into())])).unwrap(),
            root.join("reader"),
        )
        .with_runtime_workspace(root.into(), "astrid")
    }

    fn saved(root: &Path, response: &str, timestamp: &str) -> SavedJournal {
        fs::create_dir_all(root.join("journal")).unwrap();
        let document = super::super::render_astrid_journal_document(
            response, "daydream", 68.0, timestamp, None,
        );
        let path = super::super::write_collision_safe_journal_document(
            &root.join("journal"),
            "daydream",
            timestamp,
            &document,
        )
        .unwrap();
        SavedJournal::exact(path, &document, response).unwrap()
    }

    #[test]
    fn journal_continuation_reserves_exact_identity_and_transfers_only_on_explicit_choice() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let reservation =
            ReservedJournal::reserve(reader(&root), "host-exchange-17", "daydream").unwrap();
        let retry =
            ReservedJournal::reserve(reader(&root), "host-exchange-17", "daydream").unwrap();
        assert_eq!(reservation.entry_id, retry.entry_id);
        assert!(reservation.hint().contains(&format!(
            "NEXT: WRITE FROM_JOURNAL {}",
            reservation.entry_id
        )));
        assert!(!root.join("writing").exists());
        assert!(!root.join("reader/writing").exists());
        let action = format!("WRITE FROM_JOURNAL {}", reservation.entry_id);
        let r = reader(&root);
        assert!(
            r.prepare_once(
                "unpublished-selection",
                &r.preparation_revision().unwrap(),
                &action
            )
            .is_err()
        );
        let prose = format!("  Exact authored caf\u{e9}.\n{action}\nNEXT: REST");
        let published = saved(&root, &prose, "42");
        let path = published.source_path.clone();
        let original = fs::read(&path).unwrap();
        reservation.publish("daydream", &prose, published).unwrap();
        assert!(!root.join("reader/writing").exists());
        let output = r
            .prepare_once(
                "explicit-selection",
                &r.preparation_revision().unwrap(),
                &action,
            )
            .unwrap();
        assert!(output.generation_requested);
        let state: Value =
            serde_json::from_slice(&fs::read(root.join("reader/writing/drafts-v2.json")).unwrap())
                .unwrap();
        assert_eq!(state["drafts"].as_object().unwrap().len(), 1);
        assert_eq!(
            state["drafts"]["d1"]["parts"][0],
            prose.trim_end_matches("\nNEXT: REST")
        );
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn journal_continuation_excludes_nonpublic_modes_and_never_claims_a_failed_save() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for mode in [
            "mirror",
            "private_writing",
            "private_writing_notice",
            "source_study_runtime_notice",
            "dialogue_fallback",
            "daydream_longform",
        ] {
            assert!(ReservedJournal::reserve(reader(&root), "host-id", mode).is_err());
        }
        assert!(!root.join("reader").exists());
        let reservation = ReservedJournal::reserve(reader(&root), "host-id", "daydream").unwrap();
        publish_saved(Some(&reservation), "daydream", "not saved", None);
        let r = reader(&root);
        assert!(
            r.prepare_once(
                "not-published",
                &r.preparation_revision().unwrap(),
                &format!("WRITE FROM_JOURNAL {}", reservation.entry_id)
            )
            .is_err()
        );
    }

    #[test]
    fn journal_continuation_binds_collision_path_file_hash_and_response_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let original = saved(&root, "Other prose.", "42");
        let reservation =
            ReservedJournal::reserve(reader(&root), "host-second", "daydream").unwrap();
        let published = saved(&root, "Exact second prose.", "42");
        assert_ne!(published.source_path, original.source_path);
        assert!(
            published
                .source_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("collision")
        );
        let path = published.source_path.clone();
        fs::write(&path, "changed after save").unwrap();
        assert!(
            reservation
                .publish("daydream", "Exact second prose.", published)
                .is_err()
        );
        assert!(
            reservation
                .publish(
                    "witness",
                    "Exact response.",
                    saved(&root, "Exact response.", "43")
                )
                .is_err()
        );
        assert!(
            reservation
                .publish(
                    "daydream",
                    "Altered response.",
                    saved(&root, "Exact response.", "44")
                )
                .is_err()
        );
        let normalized = super::super::render_astrid_journal_document(
            "Authored.<end_of_turn>",
            "daydream",
            68.0,
            "45",
            None,
        );
        assert!(
            SavedJournal::exact(
                root.join("journal/daydream_45.txt"),
                &normalized,
                "Authored.<end_of_turn>"
            )
            .is_none()
        );
    }

    #[test]
    fn journal_continuation_ordinary_modes_have_independent_exact_operation_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let first = ReservedJournal::reserve(reader(&root), "host-same", "dialogue_live").unwrap();
        let second = ReservedJournal::reserve(reader(&root), "host-same", "aspiration").unwrap();
        let third = ReservedJournal::reserve(reader(&root), "host-next", "dialogue_live").unwrap();
        assert_ne!(first.entry_id, second.entry_id);
        assert_ne!(first.entry_id, third.entry_id);
        assert!(first.hint().len() < 1024);
        let metadata: Value =
            serde_json::from_slice(&fs::read(root.join("reader/journal-sources-v1.json")).unwrap())
                .unwrap();
        let body: Value = serde_json::from_str(metadata["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["entries"].as_array().unwrap().len(), 3);
        assert!(
            body["entries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["publication"] == json!(null))
        );
    }
}
