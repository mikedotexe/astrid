//! Operator-only, read-only access to committed private writing checkpoints.
//!
//! This deliberately does not call `Writer`, preparation recovery, or native lock
//! acquisition. The owner directory, owner envelope, downgrade guard, checkpoint,
//! and existing `reader.lock` must already exist. Schemas 4 and 5 are read as stored; no
//! migration, delivery artifact, model, or network access is performed.
//!
//! A revision selection is a SHA-256 snapshot token, not the numeric prose revision
//! and not the observation API's draft hash. It binds the exact owner envelope,
//! downgrade guard, and entire checkpoint. Even an unrelated draft edit requires
//! a new listing. JSON strings preserve authored bytes, including part boundaries.

use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata},
    io::Read as _,
    path::{Component, Path, PathBuf},
};

/// Hard ceiling; larger stores are refused, never partially parsed or truncated.
pub const MAX_CHECKPOINT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PAGE_SIZE: usize = 20;
const OWNER: &str = "preparation-owner-v1.json";
const JOURNAL: &str = "preparation-transaction-v1.json";
const CHECKPOINT: &str = "writing/drafts-v2.json";
const GUARD: &str = "writing/drafts-v1.json";
const DOWNGRADE_GUARD: &[u8] =
    b"{\"requires_private_writing_schema\":2,\"checkpoint\":\"drafts-v2.json\"}\n";

/// An explicit operator section selection. No selection combines private sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Passages,
    Questions,
    References,
    StoppingPoint,
    Annotations,
}

#[derive(Debug, Serialize)]
pub struct DraftMetadata {
    pub id: String,
    pub revision: u64,
    pub status: DraftStatus,
    pub part_count: usize,
    pub part_bytes: Vec<usize>,
    pub total_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Active,
    Parked,
    Finished,
}

#[derive(Debug, Serialize)]
pub struct DraftPage {
    pub owner: String,
    pub snapshot_sha256: String,
    pub schema_version: u32,
    pub page: usize,
    pub page_size: usize,
    pub total_drafts: usize,
    pub next_page: Option<usize>,
    pub drafts: Vec<DraftMetadata>,
}

#[derive(Debug, Serialize)]
pub struct ShownDraft {
    pub owner: String,
    pub draft: String,
    pub revision: u64,
    pub snapshot_sha256: String,
    #[serde(flatten)]
    pub content: SectionContent,
}

#[derive(Debug, Serialize)]
#[serde(tag = "section", rename_all = "snake_case")]
pub enum SectionContent {
    Passages { parts: Vec<String> },
    Questions { text: String },
    References { text: String },
    StoppingPoint { text: String },
    Annotations { annotations: Vec<Annotation> },
}

/// Only the authored annotation, not capture data, previews, or raw record bodies.
#[derive(Debug, Serialize)]
pub struct Annotation {
    pub id: String,
    pub target: String,
    pub kind: &'static str,
    pub text: String,
}

/// A capability for an explicitly selected owner's existing checkpoint directory.
/// Construction performs no I/O and retains no prose between requests.
pub struct ResearchReader {
    directory: PathBuf,
    owner: String,
}

impl ResearchReader {
    #[must_use]
    pub fn new(directory: PathBuf, owner: String) -> Self {
        Self { directory, owner }
    }

    /// List metadata only, in numeric draft-ID order; pages are one-based.
    /// # Errors
    /// Refuses invalid paging, missing/corrupt/unsupported or changing stores,
    /// symlinks, pending transactions, and absent or mismatching owner bindings.
    pub fn list(&self, page: usize, page_size: usize) -> Result<DraftPage> {
        ensure!(
            page > 0 && (1..=MAX_PAGE_SIZE).contains(&page_size),
            "invalid research page"
        );
        let offset = page
            .checked_sub(1)
            .and_then(|n| n.checked_mul(page_size))
            .context("research page overflow")?;
        self.inspect(|state, snapshot| {
            ensure!(
                offset < state.drafts.len() || (offset == 0 && state.drafts.is_empty()),
                "research page out of range"
            );
            let mut ordered: Vec<_> = state.drafts.iter().collect();
            ordered.sort_by_key(|(id, _)| draft_number(id).unwrap_or_default());
            let drafts = ordered
                .into_iter()
                .skip(offset)
                .take(page_size)
                .map(|(id, draft)| {
                    let part_bytes: Vec<_> = draft.parts.iter().map(String::len).collect();
                    Ok(DraftMetadata {
                        id: id.clone(),
                        revision: draft.revision,
                        status: if draft.finished {
                            DraftStatus::Finished
                        } else if state.active.as_ref() == Some(id) {
                            DraftStatus::Active
                        } else {
                            DraftStatus::Parked
                        },
                        part_count: draft.parts.len(),
                        total_bytes: part_bytes.iter().try_fold(0_usize, |sum, n| {
                            sum.checked_add(*n).context("draft byte count overflow")
                        })?,
                        part_bytes,
                    })
                })
                .collect::<Result<_>>()?;
            Ok(DraftPage {
                owner: self.owner.clone(),
                snapshot_sha256: snapshot.into(),
                schema_version: state.schema_version,
                page,
                page_size,
                total_drafts: state.drafts.len(),
                next_page: (offset.saturating_add(page_size) < state.drafts.len())
                    .then(|| page.saturating_add(1)),
                drafts,
            })
        })
    }

    /// Return only the requested section at the exact current snapshot.
    /// # Errors
    /// In addition to list failures, refuses stale tokens, unknown draft IDs, and
    /// hidden-model or raw-wire markers. Authored text is never sanitized here.
    pub fn show(
        &self,
        draft_id: &str,
        snapshot_sha256: &str,
        section: Section,
    ) -> Result<ShownDraft> {
        ensure!(
            draft_number(draft_id).is_some(),
            "invalid research draft ID"
        );
        ensure!(is_hash(snapshot_sha256), "invalid research snapshot token");
        self.inspect(|state, snapshot| {
            ensure!(
                snapshot == snapshot_sha256,
                "stale research snapshot; list and select again"
            );
            let draft = state
                .drafts
                .get(draft_id)
                .context("research draft not found")?;
            let content = match section {
                Section::Passages => {
                    for part in &draft.parts {
                        visible_authored_text(part)?;
                    }
                    SectionContent::Passages {
                        parts: draft.parts.clone(),
                    }
                },
                Section::Questions => SectionContent::Questions {
                    text: selected_text(&draft.question)?,
                },
                Section::References => SectionContent::References {
                    text: selected_text(&draft.evidence)?,
                },
                Section::StoppingPoint => SectionContent::StoppingPoint {
                    text: selected_text(&draft.stopping_point)?,
                },
                Section::Annotations => SectionContent::Annotations {
                    annotations: annotations(draft)?,
                },
            };
            Ok(ShownDraft {
                owner: self.owner.clone(),
                draft: draft_id.into(),
                revision: draft.revision,
                snapshot_sha256: snapshot.into(),
                content,
            })
        })
    }

    fn inspect<T>(&self, select: impl FnOnce(&State, &str) -> Result<T>) -> Result<T> {
        ensure!(
            matches!(self.owner.as_str(), "astrid" | "minime"),
            "unsupported research owner"
        );
        let guard = ReadGuard::acquire(&self.directory)?;
        let owner = guard.read(OWNER, 4096)?;
        let envelope: OwnerEnvelope = decode(&owner.bytes)?;
        ensure!(
            crate::digest(&envelope.body) == envelope.sha256,
            "invalid research owner envelope"
        );
        let saved_owner: String = decode(envelope.body.as_bytes())?;
        ensure!(saved_owner == self.owner, "research owner mismatch");
        let downgrade = guard.read(GUARD, 4096)?;
        ensure!(
            downgrade.bytes == DOWNGRADE_GUARD,
            "research downgrade guard mismatch"
        );
        let checkpoint = guard.read(CHECKPOINT, MAX_CHECKPOINT_BYTES)?;
        let state: State = decode(&checkpoint.bytes)?;
        state.validate(&self.owner)?;
        let snapshot = crate::digest(serde_json::to_vec(&(
            "writing-research-snapshot-v1",
            &self.owner,
            crate::digest(&owner.bytes),
            crate::digest(&downgrade.bytes),
            crate::digest(&checkpoint.bytes),
        ))?);
        let result = select(&state, &snapshot)?;
        guard.verify(&[(OWNER, owner), (GUARD, downgrade), (CHECKPOINT, checkpoint)])?;
        Ok(result)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerEnvelope {
    body: String,
    sha256: String,
}

// A separate read schema prevents accidentally invoking migration or recovery.
// Non-authorial delivery fields are typed but never surfaced or followed on disk.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema_version: u32,
    #[serde(rename = "sequence")]
    _sequence: u64,
    active: Option<String>,
    #[serde(deserialize_with = "unique_map")]
    drafts: BTreeMap<String, Draft>,
    #[serde(rename = "pending")]
    _pending: Option<Pending>,
    #[serde(rename = "receipts")]
    _receipts: BTreeMap<String, crate::DeliveryReceipt>,
    #[serde(default, rename = "last_choice")]
    _last_choice: Option<crate::response_choice::ChoiceReceipt>,
    #[serde(default, rename = "retained_pending")]
    _retained_pending: BTreeMap<String, Pending>,
    #[serde(default, rename = "preview_deliveries")]
    _preview_deliveries: BTreeMap<String, String>,
    #[serde(default)]
    operations: BTreeMap<String, OperationReceipt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationReceipt {
    #[serde(rename = "operation")]
    _operation: String,
    #[serde(rename = "draft")]
    _draft: Option<String>,
    #[serde(rename = "revision")]
    _revision: Option<u64>,
    #[serde(rename = "generation_requested")]
    _generation_requested: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    #[serde(rename = "topic")]
    _topic: String,
    question: String,
    evidence: String,
    parent: Option<String>,
    finished: bool,
    revision: u64,
    parts: Vec<String>,
    #[serde(default)]
    stopping_point: String,
    #[serde(default)]
    observations: crate::observations::History,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    #[serde(rename = "output")]
    _output: crate::StudyOutput,
    #[serde(rename = "draft")]
    _draft: Option<String>,
    #[serde(rename = "revision")]
    _revision: u64,
    #[serde(rename = "replace")]
    _replace: bool,
    #[serde(default, rename = "selected_action_sha256")]
    _selected_action_sha256: Option<String>,
    #[serde(default, rename = "observation_draft")]
    _observation_draft: Option<String>,
    #[serde(default, rename = "observation_preview")]
    _observation_preview: Option<String>,
}

impl State {
    fn validate(&self, owner: &str) -> Result<()> {
        ensure!(
            matches!(self.schema_version, 4 | 5),
            "unsupported research writing schema; no migration"
        );
        ensure!(
            self.schema_version == 5 || self.operations.is_empty(),
            "invalid research schema downgrade"
        );
        ensure!(
            self.drafts.len() <= 4096,
            "research draft capacity exceeded"
        );
        ensure!(
            self.active
                .as_ref()
                .is_none_or(|id| self.drafts.contains_key(id)),
            "invalid research active draft"
        );
        for (id, draft) in &self.drafts {
            ensure!(
                draft_number(id).is_some() && draft.parts.len() <= 4096,
                "invalid research draft metadata"
            );
            ensure!(
                draft
                    .parent
                    .as_ref()
                    .is_none_or(|parent| parent != id && self.drafts.contains_key(parent)),
                "invalid research draft parent"
            );
            draft
                .observations
                .check_owner(owner)
                .map_err(|_| anyhow::anyhow!("research observation owner mismatch"))?;
            draft
                .observations
                .validate(true)
                .map_err(|_| anyhow::anyhow!("corrupt research observation history"))?;
        }
        Ok(())
    }
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    // Deserialize errors can quote field names or enum values from private data.
    serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("invalid research checkpoint JSON"))
}

fn unique_map<'de, D, T>(deserializer: D) -> std::result::Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct UniqueMap<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for UniqueMap<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with unique keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry()? {
                if result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate checkpoint key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(UniqueMap(std::marker::PhantomData))
}

fn draft_number(id: &str) -> Option<u64> {
    let number = id.strip_prefix('d')?;
    if number.starts_with('0') || !number.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    number.parse().ok().filter(|n| *n > 0)
}

fn is_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn selected_text(text: &str) -> Result<String> {
    visible_authored_text(text)?;
    Ok(text.into())
}

fn annotations(draft: &Draft) -> Result<Vec<Annotation>> {
    draft
        .observations
        .records
        .iter()
        .filter_map(|record| {
            let Ok(entry) = record.entry() else {
                return Some(Err(anyhow::anyhow!("corrupt research annotation")));
            };
            let (target, text, kind) = match entry {
                crate::observations::Entry::Annotation { target, text } => {
                    (target, text, "annotation")
                },
                crate::observations::Entry::Expectation { target, text, .. } => {
                    (target, text, "expectation")
                },
                _ => return None,
            };
            Some(visible_authored_text(&text).map(|()| Annotation {
                id: record.id.clone(),
                target,
                kind,
                text,
            }))
        })
        .collect()
}

fn visible_authored_text(text: &str) -> Result<()> {
    let lower = text.to_ascii_lowercase();
    let hidden_tag = lower.split('<').skip(1).any(|tail| {
        let tag = tail.trim_start().trim_start_matches('/').trim_start();
        [
            "think",
            "thinking",
            "analysis",
            "reasoning",
            "redacted_reasoning",
            "writing_mode",
            "denial_record",
            "internal",
            "scratchpad",
        ]
        .iter()
        .any(|name| {
            tag.strip_prefix(name).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(|c: char| c == '>' || c == '/' || c.is_ascii_whitespace())
            })
        })
    });
    let wire_marker = [
        "request_json",
        "response_json",
        "reasoning_content",
        "reasoning_details",
        "<|",
        "[im_start]",
        "[im_end]",
        "[start_header_id]",
        "[end_header_id]",
        "[begin_of_text]",
        "[end_of_text]",
        "[eot_id]",
        "[thinking]",
        "[/thinking]",
        "[analysis]",
        "[/analysis]",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let wire_json = [
        "messages",
        "choices",
        "message",
        "request",
        "response",
        "system_prompt",
        "thinking",
        "reasoning",
    ]
    .iter()
    .any(|key| {
        lower
            .split(&format!("\"{key}\""))
            .skip(1)
            .any(|tail| tail.trim_start().starts_with(':'))
    });
    ensure!(
        !hidden_tag && !wire_marker && !wire_json,
        "selected section contains hidden-model or raw-wire markers; exact output refused"
    );
    Ok(())
}

struct ReadFile {
    bytes: Vec<u8>,
    metadata: Metadata,
}
struct ReadGuard {
    root: PathBuf,
    directories: Vec<(PathBuf, Metadata)>,
    lock: File,
    lock_metadata: Metadata,
}

impl ReadGuard {
    fn acquire(directory: &Path) -> Result<Self> {
        let root = if directory.is_absolute() {
            directory.into()
        } else {
            std::env::current_dir()
                .context("research working directory unavailable")?
                .join(directory)
        };
        let directories = inspect_directories(&root.join("writing"))?;
        no_transaction(&root)?;
        let (lock, lock_metadata) = open_regular(&root.join("reader.lock"), 4096)?;
        fs2::FileExt::try_lock_shared(&lock)
            .map_err(|_| anyhow::anyhow!("research owner store is busy"))?;
        let guard = Self {
            root,
            directories,
            lock,
            lock_metadata,
        };
        guard.verify_paths()?;
        Ok(guard)
    }

    fn read(&self, relative: &str, maximum: u64) -> Result<ReadFile> {
        self.verify_paths()?;
        read_file(&self.root.join(relative), maximum)
    }

    fn verify(&self, files: &[(&str, ReadFile)]) -> Result<()> {
        self.verify_paths()?;
        for (name, original) in files {
            let current = self.read(name, MAX_CHECKPOINT_BYTES)?;
            ensure!(
                same_metadata(&original.metadata, &current.metadata)
                    && original.bytes == current.bytes,
                "research checkpoint changed during read"
            );
        }
        self.verify_paths()
    }

    fn verify_paths(&self) -> Result<()> {
        no_transaction(&self.root)?;
        for (path, original) in &self.directories {
            let current = fs::symlink_metadata(path).context("research directory unavailable")?;
            ensure!(
                current.is_dir() && same_identity(original, &current),
                "research directory changed or is a symlink"
            );
            if path == &self.root || path == &self.root.join("writing") {
                ensure!(
                    same_metadata(original, &current),
                    "research directory changed during read"
                );
            }
        }
        let current = fs::symlink_metadata(self.root.join("reader.lock"))
            .context("research lock unavailable")?;
        ensure!(
            current.is_file()
                && same_metadata(&self.lock_metadata, &current)
                && same_metadata(&self.lock_metadata, &self.lock.metadata()?),
            "research lock changed during read"
        );
        Ok(())
    }
}

fn inspect_directories(path: &Path) -> Result<Vec<(PathBuf, Metadata)>> {
    let mut current = PathBuf::new();
    let mut result = Vec::new();
    for component in path.components() {
        ensure!(
            matches!(component, Component::RootDir | Component::Normal(_)),
            "research path must not contain traversal"
        );
        current.push(component);
        let metadata = fs::symlink_metadata(&current).context("research directory unavailable")?;
        ensure!(
            metadata.is_dir(),
            "research path contains a symlink or non-directory"
        );
        result.push((current.clone(), metadata));
    }
    Ok(result)
}

fn no_transaction(root: &Path) -> Result<()> {
    match fs::symlink_metadata(root.join(JOURNAL)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => bail!("research transaction status unavailable"),
        Ok(_) => bail!("pending preparation transaction; research will not recover it"),
    }
}

fn open_regular(path: &Path, maximum: u64) -> Result<(File, Metadata)> {
    let before = fs::symlink_metadata(path).context("required research file unavailable")?;
    ensure!(
        before.is_file() && before.len() <= maximum,
        "research file is nonregular, symlinked, or oversized"
    );
    let file = File::open(path).context("research file unavailable")?;
    let after = file
        .metadata()
        .context("research file metadata unavailable")?;
    ensure!(
        same_metadata(&before, &after),
        "research file changed while opening"
    );
    let current = fs::symlink_metadata(path).context("research file disappeared")?;
    ensure!(
        current.is_file() && same_metadata(&after, &current),
        "research file changed while opening"
    );
    Ok((file, after))
}

fn read_file(path: &Path, maximum: u64) -> Result<ReadFile> {
    let (file, metadata) = open_regular(path, maximum)?;
    let mut bytes = Vec::new();
    (&file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .context("research file read failed")?;
    ensure!(
        u64::try_from(bytes.len())? == metadata.len()
            && metadata.len() <= maximum
            && same_metadata(&metadata, &file.metadata()?),
        "research file changed or exceeded its bound"
    );
    let current = fs::symlink_metadata(path).context("research file disappeared")?;
    ensure!(
        current.is_file() && same_metadata(&metadata, &current),
        "research file changed during read"
    );
    Ok(ReadFile { bytes, metadata })
}

fn same_metadata(a: &Metadata, b: &Metadata) -> bool {
    same_identity(a, b)
        && a.len() == b.len()
        && a.modified().ok() == b.modified().ok()
        && change_time_matches(a, b)
}

#[cfg(unix)]
fn same_identity(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
}
#[cfg(unix)]
fn change_time_matches(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    a.ctime() == b.ctime() && a.ctime_nsec() == b.ctime_nsec() && a.nlink() == b.nlink()
}
#[cfg(not(unix))]
fn same_identity(a: &Metadata, b: &Metadata) -> bool {
    a.file_type() == b.file_type()
        && a.permissions().readonly() == b.permissions().readonly()
        && a.created().ok() == b.created().ok()
}
#[cfg(not(unix))]
fn change_time_matches(_a: &Metadata, _b: &Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistency_check_refuses_mutation_after_the_initial_read() {
        for change in [
            "checkpoint",
            "owner",
            "guard",
            "journal",
            "lock",
            "directory",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().canonicalize().unwrap();
            fs::create_dir(root.join("writing")).unwrap();
            for name in ["reader.lock", OWNER, GUARD, CHECKPOINT] {
                fs::write(root.join(name), b"before").unwrap();
            }
            let guard = ReadGuard::acquire(&root).unwrap();
            let files = [
                (OWNER, guard.read(OWNER, 4096).unwrap()),
                (GUARD, guard.read(GUARD, 4096).unwrap()),
                (CHECKPOINT, guard.read(CHECKPOINT, 4096).unwrap()),
            ];
            match change {
                "checkpoint" => fs::write(root.join(CHECKPOINT), b"after!").unwrap(),
                "owner" => fs::write(root.join(OWNER), b"after!").unwrap(),
                "guard" => fs::write(root.join(GUARD), b"after!").unwrap(),
                "journal" => fs::write(root.join(JOURNAL), b"pending").unwrap(),
                "lock" => {
                    fs::rename(root.join("reader.lock"), root.join("old.lock")).unwrap();
                    fs::write(root.join("reader.lock"), b"before").unwrap();
                },
                "directory" => {
                    fs::rename(root.join("writing"), root.join("old-writing")).unwrap();
                    fs::create_dir(root.join("writing")).unwrap();
                },
                _ => unreachable!(),
            }
            assert!(guard.verify(&files).is_err(), "undetected {change}");
        }
    }
}
