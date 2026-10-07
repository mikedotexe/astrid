//! Publication deduplication only. Selection, codec admission and rereading are unchanged.
use crate::journal::MirrorExcerpt;
use fs2::FileExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Serialize)]
pub(super) struct Publication {
    pub schema: &'static str,
    pub state: &'static str,
    pub artifact: String,
    pub rendered_sha256: String,
    pub retained_preparation_count: usize,
}

pub(super) fn retain(
    journal_dir: &Path,
    source_ref: &str,
    excerpt: &MirrorExcerpt,
    rendered: &str,
    timestamp: &str,
) -> io::Result<Publication> {
    let rendered_sha256 = format!("{:x}", Sha256::digest(rendered.as_bytes()));
    let evidence = serde_json::json!({
        "schema": "peer_mirror_record_v1",
        "excerpt": excerpt,
        "rendered_sha256": rendered_sha256,
        "context_sanitization": "peer-action/status projection applied before rendering",
        "authorship": "minime_owned_not_astrid_authored",
    });
    let evidence_line = format!("Mirror-evidence: {}", serde_json::to_string(&evidence)?);
    let identity = format!("{:x}", Sha256::digest(evidence_line.as_bytes()));
    let directory = journal_dir.join("peer_mirrors");
    std::fs::create_dir_all(&directory)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".publication.lock"))?;
    lock.lock_exclusive()?;
    let filename = format!("peer_mirror_v1_{identity}.txt");
    let path = directory.join(&filename);
    let state = match std::fs::read_to_string(&path) {
        Ok(existing) => {
            let Some((header, body)) = existing.split_once("\n\n") else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "incomplete peer mirror record; preserved",
                ));
            };
            if !header.starts_with("=== PEER MIRROR EXCERPT ===\n")
                || header
                    .lines()
                    .filter(|line| line.starts_with("Mirror-evidence:"))
                    .collect::<Vec<_>>()
                    != [evidence_line.as_str()]
                || body != format!("{rendered}\n")
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "peer mirror identity/content mismatch; preserved",
                ));
            }
            "already_retained"
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let source = serde_json::to_string(&format!("minime_journal:{source_ref}"))?;
            let document = format!(
                "=== PEER MIRROR EXCERPT ===\nMode: mirror\nAuthorship: minime_owned_reflected_without_reauthoring\nSource-ID-JSON: {source}\nFirst-retained-at: {timestamp}\n{evidence_line}\nBoundary: a bounded normalized peer excerpt, not a new Astrid entry or a complete source copy; repeated exchanges refer to this artifact\n\n{rendered}\n"
            );
            publish_complete(&directory, &filename, document.as_bytes())?;
            "newly_retained"
        },
        Err(error) => return Err(error),
    };
    let preparation_prefix = format!(".{filename}.prepared-");
    let retained_preparation_count = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(&preparation_prefix)
        })
        .count();
    Ok(Publication {
        schema: "peer_mirror_publication_v1",
        state,
        artifact: format!("journal/peer_mirrors/{filename}"),
        rendered_sha256,
        retained_preparation_count,
    })
}

fn publish_complete(directory: &Path, filename: &str, document: &[u8]) -> io::Result<()> {
    for slot in 0_u16..1024 {
        let prepared = directory.join(format!(".{filename}.prepared-{slot}"));
        let mut file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&prepared)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        // Incomplete preparations remain evidence, but cannot occupy the final name.
        file.write_all(document)?;
        file.sync_all()?;
        File::open(directory)?.sync_all()?;
        // Atomic no-clobber publication on the same filesystem, unlike rename-overwrite.
        std::fs::hard_link(&prepared, directory.join(filename))?;
        File::open(directory)?.sync_all()?;
        if std::fs::remove_file(&prepared).is_ok() {
            File::open(directory)?.sync_all()?;
        }
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "peer mirror preparation budget exhausted; prior preparations preserved",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(root: &Path, suffix: &str) -> MirrorExcerpt {
        let path = root.join("source.txt");
        std::fs::write(
            &path,
            format!("{} {suffix}", "Peer expression. ".repeat(300)),
        )
        .unwrap();
        MirrorExcerpt::read(&path).unwrap()
    }

    #[test]
    fn repeats_across_restarts_and_renames_reference_one_immutable_peer_excerpt() {
        let root = tempfile::tempdir().unwrap();
        let excerpt = input(root.path(), "first ending");
        let journal = root.path().join("journal");
        let first = retain(&journal, "source.txt", &excerpt, &excerpt.text, "42").unwrap();
        let artifact = root.path().join(&first.artifact);
        let bytes = std::fs::read(&artifact).unwrap();
        for (name, time) in [("source.txt", "81"), ("!source.txt", "120")] {
            let next = retain(&journal, name, &excerpt, &excerpt.text, time).unwrap();
            assert_eq!(next.state, "already_retained");
            assert_eq!(next.artifact, first.artifact);
            assert_eq!(std::fs::read(&artifact).unwrap(), bytes);
        }
        assert_eq!(first.state, "newly_retained");
        assert!(crate::journal::read_local_journal_body_for_continuity(&artifact).is_none());
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"excerpt_truncated\":true"));
        assert!(text.contains("not a new Astrid entry"));
        assert!(!text.contains("Fill:"));
        assert_eq!(
            text.split_once("\n\n").unwrap().1,
            format!("{}\n", excerpt.text)
        );
    }

    #[test]
    fn changed_source_tail_or_projection_is_not_suppressed() {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("journal");
        let one = input(root.path(), "one");
        let first = retain(&journal, "source.txt", &one, &one.text, "1").unwrap();
        let two = input(root.path(), "two");
        assert_eq!(one.text, two.text);
        let second = retain(&journal, "source.txt", &two, &two.text, "2").unwrap();
        assert_ne!(first.artifact, second.artifact);
        let third = retain(&journal, "source.txt", &two, "Changed projection", "3").unwrap();
        assert_ne!(second.artifact, third.artifact);
    }

    #[test]
    fn simultaneous_publishers_do_not_create_duplicate_entries() {
        let root = tempfile::tempdir().unwrap();
        let excerpt = input(root.path(), "shared");
        let journal = root.path().join("journal");
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        retain(&journal, "source.txt", &excerpt, &excerpt.text, "42").unwrap()
                    })
                })
                .collect();
            let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            assert_eq!(
                results
                    .iter()
                    .filter(|r| r.state == "newly_retained")
                    .count(),
                1
            );
            assert!(results.iter().all(|r| r.artifact == results[0].artifact));
        });
    }

    #[test]
    fn partial_or_tampered_records_fail_without_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let excerpt = input(root.path(), "shared");
        let journal = root.path().join("journal");
        let first = retain(&journal, "source.txt", &excerpt, &excerpt.text, "42").unwrap();
        let path = root.path().join(&first.artifact);
        let original = std::fs::read_to_string(&path).unwrap();
        for corrupt in [
            "partial".to_owned(),
            original.replace("Peer expression.", "Altered expression."),
        ] {
            std::fs::write(&path, &corrupt).unwrap();
            let err = retain(&journal, "source.txt", &excerpt, &excerpt.text, "43").unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), corrupt);
        }
    }

    #[test]
    fn interrupted_preparation_does_not_block_later_complete_publication() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path();
        let pending = directory.join(".record.txt.prepared-0");
        std::fs::write(&pending, "incomplete").unwrap();
        publish_complete(directory, "record.txt", b"complete").unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("record.txt")).unwrap(),
            "complete"
        );
        assert_eq!(std::fs::read_to_string(pending).unwrap(), "incomplete");
        assert!(!directory.join(".record.txt.prepared-1").exists());
        assert!(publish_complete(directory, "record.txt", b"must not overwrite").is_err());
        assert_eq!(
            std::fs::read_to_string(directory.join("record.txt")).unwrap(),
            "complete"
        );
    }

    #[test]
    fn restart_after_publication_reports_preparation_cleanup_debt() {
        let root = tempfile::tempdir().unwrap();
        let excerpt = input(root.path(), "shared");
        let journal = root.path().join("journal");
        let first = retain(&journal, "source.txt", &excerpt, &excerpt.text, "42").unwrap();
        let path = root.path().join(&first.artifact);
        let pending = path.with_file_name(format!(
            ".{}.prepared-0",
            path.file_name().unwrap().to_str().unwrap()
        ));
        std::fs::hard_link(&path, &pending).unwrap();
        let retry = retain(&journal, "source.txt", &excerpt, &excerpt.text, "43").unwrap();
        assert_eq!(retry.state, "already_retained");
        assert_eq!(retry.retained_preparation_count, 1);
        assert!(pending.exists());
    }
}
