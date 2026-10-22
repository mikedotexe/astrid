//! Optional metadata presentation, never letter delivery or generation authority.
use astrid_source_study::owner_transaction::OwnerTransaction;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, &'static str>;
const MAX_BYTES: usize = 65_536;
const MAX_REVISIONS: usize = 256;
const TTL: u64 = 48 * 3600;
const HISTORY_SCHEMA: &str = "astrid.steward_invitation_presentations.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Status,
    Defer,
    Decline,
}

pub(super) fn parse(action: &str) -> Result<Command> {
    let words: Vec<_> = action.split_whitespace().collect();
    if words.len() != 2 || !words[0].eq_ignore_ascii_case("STEWARD_INVITATION") {
        return Err("invalid_invitation_command");
    }
    match words[1].to_ascii_uppercase().as_str() {
        "STATUS" => Ok(Command::Status),
        "DEFER" => Ok(Command::Defer),
        "DECLINE" => Ok(Command::Decline),
        _ => Err("invalid_invitation_command"),
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    subject: String,
    ts: u64,
    file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    packet_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    packet_items: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary_topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary_topic_gravity: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intent_summary: Option<String>,
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Slot {
    fn validate(&self) -> Result<()> {
        if self.subject.trim().is_empty()
            || self.subject.chars().count() > 80
            || self.file.is_empty()
            || self.file.len() > 1024
            || self
                .content_sha256
                .as_ref()
                .is_some_and(|hash| !is_hash(hash))
            || self
                .review_target
                .as_ref()
                .is_some_and(|target| target.chars().count() > 120)
            || [&self.packet_mode, &self.primary_topic, &self.intent_summary]
                .iter()
                .any(|value| {
                    value
                        .as_ref()
                        .is_some_and(|text| text.chars().count() > 220)
                })
        {
            return Err("invalid_invitation_metadata");
        }
        Ok(())
    }

    fn metadata(&self) -> Value {
        json!({"subject":self.subject, "review_target":self.review_target,
            "packet_mode":self.packet_mode, "packet_items":self.packet_items,
            "primary_topic":self.primary_topic, "primary_topic_gravity":self.primary_topic_gravity,
            "intent_summary":self.intent_summary})
    }

    fn revision(&self) -> String {
        let mut material = self.metadata();
        material["content_sha256"] = json!(self.content_sha256);
        hash(&serde_json::to_vec(&material).expect("bounded metadata serializes"))
    }

    fn text(&self) -> String {
        format!(
            "Optional steward invitation (reference, not a task): {}. STEWARD_INVITATION STATUS retrieves this metadata; STEWARD_INVITATION DEFER or STEWARD_INVITATION DECLINE quiets this revision. No answer or action is required.",
            self.metadata()
        )
    }
}

pub(in crate::autonomous) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
pub(in crate::autonomous) fn render(value: &Value) -> Result<String> {
    let slot: Slot =
        serde_json::from_value(value.clone()).map_err(|_| "invalid_invitation_metadata")?;
    slot.validate()?;
    Ok(slot.text())
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Disposition {
    Presented,
    Deferred,
    Declined,
    Attended,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    revision: String,
    status: Disposition,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct History {
    schema: String,
    revisions: Vec<Receipt>,
}

pub(in crate::autonomous) struct Store {
    slot: PathBuf,
    history: PathBuf,
    lock_root: PathBuf,
}

impl Store {
    pub(in crate::autonomous) fn new(slot: &Path) -> Result<Self> {
        let slot = checked(slot)?;
        let parent = slot.parent().ok_or("unsafe_invitation_path")?;
        Ok(Self {
            history: parent.join("steward_invitation_presentations_v1.json"),
            lock_root: parent.join(".steward_invitation_v1"),
            slot,
        })
    }

    fn transaction(&self) -> Result<OwnerTransaction> {
        checked(&self.lock_root)?;
        checked(&self.lock_root.join("reader.lock"))?;
        let lock = OwnerTransaction::acquire(&self.lock_root)
            .map_err(|_| "invitation_lock_unavailable")?;
        for path in [&self.slot, &self.history] {
            match std::fs::symlink_metadata(path.with_extension("pending")) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                _ => return Err("invitation_publication_unresolved"),
            }
        }
        Ok(lock)
    }

    fn current(&self) -> Result<Option<Slot>> {
        let Some(bytes) = read(&self.slot)? else {
            return Ok(None);
        };
        let slot: Slot =
            serde_json::from_slice(&bytes).map_err(|_| "invalid_invitation_metadata")?;
        slot.validate()?;
        Ok(Some(slot))
    }

    fn history(&self) -> Result<History> {
        let Some(bytes) = read(&self.history)? else {
            return Ok(History {
                schema: HISTORY_SCHEMA.into(),
                revisions: vec![],
            });
        };
        let history: History =
            serde_json::from_slice(&bytes).map_err(|_| "invalid_invitation_history")?;
        let mut seen = BTreeSet::new();
        if history.schema != HISTORY_SCHEMA
            || history.revisions.len() > MAX_REVISIONS
            || history
                .revisions
                .iter()
                .any(|item| !is_hash(&item.revision) || !seen.insert(&item.revision))
        {
            return Err("invalid_invitation_history");
        }
        Ok(history)
    }

    fn mark(&self, history: &mut History, revision: &str, status: Disposition) -> Result<()> {
        if let Some(item) = history
            .revisions
            .iter_mut()
            .find(|item| item.revision == revision)
        {
            if item.status == status {
                return Ok(());
            }
            item.status = status;
        } else {
            if history.revisions.len() >= MAX_REVISIONS {
                return Err("invitation_history_capacity");
            }
            history.revisions.push(Receipt {
                revision: revision.into(),
                status,
            });
        }
        publish(&self.history, history)
    }

    pub(in crate::autonomous) fn record(&self, value: Value) -> Result<()> {
        let slot: Slot =
            serde_json::from_value(value).map_err(|_| "invalid_invitation_metadata")?;
        slot.validate()?;
        let _lock = self.transaction()?;
        self.history()?;
        if self
            .current()?
            .is_some_and(|current| current.revision() == slot.revision())
        {
            return Ok(());
        }
        publish(&self.slot, &slot)
    }

    pub(in crate::autonomous) fn offer(&self, outbox: &Path, now: u64) -> Result<Option<String>> {
        let _lock = self.transaction()?;
        let Some(slot) = self.current()? else {
            return Ok(None);
        };
        let mut history = self.history()?;
        let revision = slot.revision();
        if history
            .revisions
            .iter()
            .any(|item| item.revision == revision)
            || history.revisions.len() >= MAX_REVISIONS
            || now < slot.ts
            || now.saturating_sub(slot.ts) > TTL
            || newer_report(outbox, slot.ts)?
        {
            return Ok(None);
        }
        // Persist assembly before returning text. A crash may lose this optional
        // presentation, but must not claim delivery or repeat it after restart.
        self.mark(&mut history, &revision, Disposition::Presented)?;
        Ok(Some(slot.text()))
    }

    pub(super) fn command(&self, command: Command) -> Result<Value> {
        let _lock = self.transaction()?;
        let Some(slot) = self.current()? else {
            return Ok(json!({"status":"absent", "generation_requested":false}));
        };
        let mut history = self.history()?;
        let revision = slot.revision();
        match command {
            Command::Status => {},
            Command::Defer => self.mark(&mut history, &revision, Disposition::Deferred)?,
            Command::Decline => self.mark(&mut history, &revision, Disposition::Declined)?,
        }
        let status = history
            .revisions
            .iter()
            .find(|item| item.revision == revision)
            .map_or(json!("unpresented"), |item| json!(item.status));
        Ok(
            json!({"status":status, "revision":revision, "invitation":slot.text(),
            "generation_requested":false, "source_retained":true,
            "receipt_scope":"presentation_or_owner_metadata_not_delivery"}),
        )
    }

    /// Compare and mark under one owner transaction; never remove the retained source.
    pub(in crate::autonomous) fn attend_if(
        &self,
        matches: impl FnOnce(&str) -> bool,
    ) -> Result<()> {
        let _lock = self.transaction()?;
        let Some(slot) = self.current()? else {
            return Ok(());
        };
        let mut history = self.history()?;
        if slot.review_target.as_deref().is_some_and(matches) {
            self.mark(&mut history, &slot.revision(), Disposition::Attended)?;
        }
        Ok(())
    }
}

fn newer_report(outbox: &Path, stamp: u64) -> Result<bool> {
    let entries = match std::fs::read_dir(outbox) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("invitation_outbox_unavailable"),
    };
    for (index, entry) in entries.enumerate() {
        if index >= 4096 {
            return Err("invitation_outbox_capacity");
        }
        let entry = entry.map_err(|_| "invitation_outbox_unavailable")?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("steward_report"))
        {
            let metadata = entry
                .metadata()
                .map_err(|_| "invitation_outbox_unavailable")?;
            let modified = metadata
                .modified()
                .map_err(|_| "invitation_outbox_unavailable")?;
            if metadata.is_file()
                && modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "invitation_outbox_unavailable")?
                    .as_secs()
                    > stamp
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn checked(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("unsafe_invitation_path");
    }
    let mut path = path.to_owned();
    // macOS system temp aliases are not workspace-controlled links.
    #[cfg(target_os = "macos")]
    for alias in ["/var", "/tmp"] {
        if let Ok(rest) = path.strip_prefix(alias) {
            let expected = Path::new("/private").join(alias.trim_start_matches('/'));
            if Path::new(alias).canonicalize().ok().as_ref() != Some(&expected) {
                return Err("unsafe_invitation_path");
            }
            path = expected.join(rest);
            break;
        }
    }
    for part in path.ancestors() {
        match std::fs::symlink_metadata(part) {
            Ok(meta) if meta.file_type().is_symlink() => return Err("unsafe_invitation_path"),
            Ok(_) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err("unsafe_invitation_path"),
        }
    }
    Ok(path)
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let path = checked(path)?;
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("invitation_read_failed"),
    };
    let metadata = file.metadata().map_err(|_| "invitation_read_failed")?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > MAX_BYTES as u64 {
        return Err("invalid_invitation_file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "invitation_read_failed")?;
    if bytes.len() > MAX_BYTES {
        return Err("invalid_invitation_file");
    }
    Ok(Some(bytes))
}

fn publish(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let bytes = serde_json::to_vec(value).map_err(|_| "invalid_invitation_metadata")?;
    if bytes.len() > MAX_BYTES {
        return Err("invitation_history_capacity");
    }
    let path = checked(path)?;
    let pending = checked(&path.with_extension("pending"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&pending)
        .map_err(|_| "invitation_publication_unresolved")?;
    // Failed preparations remain bounded evidence; never overwrite or auto-recover them.
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "invitation_publication_unresolved")?;
    std::fs::rename(&pending, &path).map_err(|_| "invitation_publication_unresolved")?;
    File::open(path.parent().ok_or("unsafe_invitation_path")?)
        .and_then(|parent| parent.sync_all())
        .map_err(|_| "invitation_publication_unresolved")
}

pub(super) fn handle(action: &str, slot: &Path) -> crate::action_continuity::NextActionOutcome {
    use crate::action_continuity::NextActionOutcome;
    match parse(action).and_then(|command| Store::new(slot)?.command(command)) {
        Ok(value) => NextActionOutcome::handled("steward_invitation", value.to_string())
            .with_stage_visibility("language_only", "protected_summary"),
        Err(reason) => NextActionOutcome::blocked("steward_invitation", format!(
            "Optional invitation metadata unavailable: {reason}. Use STEWARD_INVITATION STATUS, STEWARD_INVITATION DEFER, or STEWARD_INVITATION DECLINE. No generation, reply, or continuation requested."
        )).with_stage_visibility("blocked", "protected_summary"),
    }
}

#[cfg(test)]
mod tests;
