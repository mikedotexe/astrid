//! Explicit local human addresses and completion views, without control authority.
//!
//! Filenames and leading sender declarations are a routing convention within the
//! shared workspace. They are not sender authentication or permission to act.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use sha2::{Digest as _, Sha256};

use super::durable_inbox::InboxReservation;

#[cfg(test)]
#[path = "human_correspondence_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HumanLetterKind {
    Query,
    Feedback,
}

/// Read only the initial header block. Body quotes cannot declare a sender.
pub(super) fn classify_source(path: &Path, source: &str) -> Option<HumanLetterKind> {
    let filename = path.file_name()?.to_str()?;
    if filename
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    let kind = if filename.starts_with("mike_query_") {
        HumanLetterKind::Query
    } else if filename.starts_with("mike_feedback_") {
        HumanLetterKind::Feedback
    } else {
        return None;
    };
    let mut sender = None;
    for (index, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            break;
        }
        if index == 0 && line.starts_with("=== ") && line.ends_with(" ===") {
            continue;
        }
        let (key, value) = line.split_once(':')?;
        // A quoted or indented header is not a source declaration.
        if key.trim() != key || key.is_empty() {
            return None;
        }
        if key.eq_ignore_ascii_case("from") {
            if sender.is_some() {
                return None;
            }
            sender = Some(value.trim().to_ascii_lowercase());
        }
    }
    match sender.as_deref()? {
        "mike" | "mike & claude" | "mike & codex" | "v and codex" | "v & codex" => Some(kind),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompletionParts {
    pub had_reply_blocks: bool,
    /// Ordinary signal/action text. Address-shaped spans never fall back here.
    pub residual: String,
    /// Only one complete, nonempty declaration for the admitted human identity.
    pub human_reply: Option<String>,
    language: String,
    reply_blocks_complete: bool,
}

/// Provider quality checks inspect authored language separately from native NEXT
/// declarations. The accepted and retained completion remains the original text.
pub(crate) fn human_reply_quality_views(text: &str) -> (String, String, bool) {
    let parts = split_completion(text, None);
    (parts.language, parts.residual, parts.reply_blocks_complete)
}

fn line_text(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn fence_marker(line: &str) -> Option<(u8, usize, &str)> {
    let content = line.trim_start_matches(' ');
    if line.len() - content.len() > 3 {
        return None;
    }
    let marker = *content.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = content.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length, &content[length..]))
}

fn address_shaped(line: &str) -> bool {
    let line = line.trim_start();
    let line = line.strip_prefix("NEXT:").map_or(line, str::trim_start);
    line.strip_prefix("INBOX_REPLY")
        .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
}

fn declared_id(line: &str) -> Option<&str> {
    let line = line.strip_prefix("NEXT: ").unwrap_or(line);
    let id = line.strip_prefix("INBOX_REPLY ")?;
    (!id.is_empty() && !id.chars().any(char::is_whitespace)).then_some(id)
}

/// END_INBOX_REPLY is mandatory: NEXT-looking text inside a human body remains
/// language. Unterminated, malformed and wrong-address blocks are retained only
/// in the original completion, never restored to peer prose or action parsing.
pub(super) fn split_completion(text: &str, admitted_message_id: Option<&str>) -> CompletionParts {
    let mut residual = String::new();
    let mut language = String::new();
    let mut declarations = 0_usize;
    let mut body = String::new();
    let mut candidate = None;
    let mut depth = 0_usize;
    let mut matching = false;
    let mut fence = None;
    for line in text.split_inclusive('\n') {
        let raw = line_text(line);
        if let Some((marker, length, suffix)) = fence_marker(raw) {
            match fence {
                None => fence = Some((marker, length)),
                Some((opening_marker, opening_length))
                    if marker == opening_marker
                        && length >= opening_length
                        && suffix.trim().is_empty() =>
                {
                    fence = None;
                },
                _ => {},
            }
        } else if fence.is_none() {
            if address_shaped(raw) {
                declarations = declarations.saturating_add(1);
                if depth == 0 {
                    matching =
                        admitted_message_id.is_some() && declared_id(raw) == admitted_message_id;
                    body.clear();
                } else {
                    matching = false;
                }
                depth = depth.saturating_add(1);
                continue;
            }
            if depth > 0 && raw == "END_INBOX_REPLY" {
                depth -= 1;
                if depth == 0 && matching && !body.trim().is_empty() {
                    candidate = Some(body.clone());
                }
                continue;
            }
        }
        if depth > 0 {
            body.push_str(line);
            language.push_str(line);
        } else {
            residual.push_str(line);
            if !raw.trim_start().starts_with("NEXT:") {
                language.push_str(line);
            }
        }
    }
    CompletionParts {
        had_reply_blocks: declarations > 0,
        residual,
        human_reply: (declarations == 1 && depth == 0)
            .then_some(candidate)
            .flatten(),
        language,
        reply_blocks_complete: depth == 0,
    }
}

fn checked_header(value: &str) -> Result<&str> {
    if value.is_empty() || value.chars().any(char::is_control) {
        bail!("invalid addressed reply evidence header");
    }
    Ok(value)
}

/// Called after provider receipt and durable reservation validation, while the
/// inbox commit lock is held. Publication precedes the acknowledgement so a
/// failed write remains recoverable from the retained completion.
pub(super) fn publish_reply(
    outbox_root: &Path,
    letter: &InboxReservation,
    receipt: &crate::llm::PromptDeliveryReceiptV1,
    body: &str,
) -> Result<PathBuf> {
    if classify_source(&letter.letter.source_path, &letter.text).is_none() || body.trim().is_empty()
    {
        bail!("addressed reply requires a validated human source and nonempty body");
    }
    let mid = checked_header(&letter.letter.message_id)?;
    let thread = checked_header(&letter.letter.thread_id)?;
    let version = checked_header(&letter.letter.version_id)?;
    let source_hash = checked_header(&letter.content_sha256)?;
    let completion_hash = checked_header(&receipt.retained_completion_sha256)?;
    let request_hash = checked_header(&receipt.request_sha256)?;
    let artifact_hash = checked_header(&receipt.retained_artifact_sha256)?;
    let artifact = format!(
        "=== ASTRID ADDRESSED REPLY V1 ===\n\
         To: mike\n\
         Reply-To: {mid}\n\
         Thread-Id: {thread}\n\
         Source-SHA256: {source_hash}\n\
         Source-Version: {version}\n\
         Completion-SHA256: {completion_hash}\n\
         Delivery-Request-SHA256: {request_hash}\n\
         Retained-Artifact-SHA256: {artifact_hash}\n\
         Address-Evidence: explicit INBOX_REPLY declaration; language only\n\n{body}"
    );
    let directory = outbox_root.join("human/mike");
    fs::create_dir_all(&directory)?;
    // Persist newly created directory entries as well as the eventual file.
    for parent in [directory.parent(), Some(outbox_root), outbox_root.parent()]
        .into_iter()
        .flatten()
    {
        File::open(parent)?.sync_all()?;
    }
    let destination = directory.join(format!(
        "human_reply_{:x}.txt",
        Sha256::digest(version.as_bytes())
    ));
    publish_atomic(&directory, &destination, artifact.as_bytes())?;
    Ok(destination)
}

fn verify_existing(path: &Path, expected: &[u8]) -> Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_file() || fs::read(path)? != expected {
        bail!("conflicting addressed reply artifact: {}", path.display());
    }
    File::open(path)?.sync_all()?;
    Ok(())
}

fn publish_atomic(directory: &Path, destination: &Path, bytes: &[u8]) -> Result<()> {
    // Hard-link installation is atomic and cannot overwrite a prior completion.
    // A crashed writer can leave only a hidden temp file or the complete artifact.
    let temporary = directory.join(format!(".human_reply_{:016x}.tmp", rand::random::<u64>()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs::hard_link(&temporary, destination) {
            Ok(()) => {},
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                verify_existing(destination, bytes)?;
            },
            Err(error) => return Err(error.into()),
        }
        File::open(directory)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}
