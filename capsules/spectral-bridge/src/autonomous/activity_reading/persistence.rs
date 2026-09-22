use super::{ActivityRuntimeV1, ReaderActivityRefV1, preview_reader};
use crate::action_continuity::{ActionContinuityStore, ReaderDisposition};
use anyhow::{Context, Result, anyhow};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;

const RUNTIME_FILE: &str = "activity_runtime_v3.json";
const PREVIOUS_FILE: &str = "activity_runtime_v2.json";
const LEGACY_FILE: &str = "activity_runtime_v1.json";
const PREVIOUS_GUARD: &str = "{\"foreground_reader\":\"requires activity_runtime_v3\"}\n";
const DOWNGRADE_GUARD: &str = "{\"foreground_reader\":\"requires activity_runtime_v2\"}\n";
const MAX_RUNTIME_BYTES: u64 = 32 * 1024 * 1024;

pub(super) fn validate_reader_ref(reader: &ReaderActivityRefV1) -> Result<()> {
    for value in [&reader.thread_id, &reader.session_id] {
        if value.is_empty()
            || value.len() > 512
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(anyhow!("invalid saved reader identity"));
        }
    }
    Ok(())
}

fn validate_activity(activity: &ActivityRuntimeV1) -> Result<()> {
    activity.study_handoff.validate()?;
    anyhow::ensure!(
        activity.native_focus_window.is_none() || !activity.study_handoff.pending(),
        "ordinary study and protected focus cannot own generation together"
    );
    if let Some(window) = &activity.native_focus_window {
        anyhow::ensure!(
            !window.is_empty() && window.len() <= 128,
            "invalid native focus window"
        );
        anyhow::ensure!(
            activity.foreground_reader.is_none() && activity.mailbox_window.is_none(),
            "native focus and another foreground activity cannot be selected together"
        );
    }
    for reader in [&activity.foreground_reader, &activity.return_reader]
        .into_iter()
        .flatten()
    {
        validate_reader_ref(reader)?;
    }
    if let Some(window) = &activity.mailbox_window
        && (window.id.is_empty()
            || window.id.len() > 512
            || !window
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    {
        return Err(anyhow!("invalid mailbox window identity"));
    }
    if activity.foreground_reader.is_some() && activity.mailbox_window.is_some() {
        return Err(anyhow!(
            "foreground reading and mailbox cannot be selected together"
        ));
    }
    Ok(())
}

pub(crate) fn persist_activity(
    store: &ActionContinuityStore,
    activity: &ActivityRuntimeV1,
) -> Result<ActivityRuntimeV1> {
    let _lock = store.reader_owner_transaction()?;
    validate_activity(activity)?;
    fs::create_dir_all(store.root())?;
    let current = load_activity(store)?;
    anyhow::ensure!(
        activity.selection_revision == current.selection_revision,
        "stale foreground selection; reload ACTIVITY_STATUS before updating"
    );
    let mut committed = activity.clone();
    committed.selection_revision = current
        .selection_revision
        .checked_add(1)
        .context("activity selection revision exhausted")?;
    let encoded = serde_json::to_vec_pretty(&committed)?;
    anyhow::ensure!(
        u64::try_from(encoded.len())? <= MAX_RUNTIME_BYTES,
        "activity selection exceeds size limit; prior state retained"
    );
    prepare_upgrade(store)?;
    let target = store.root().join(RUNTIME_FILE);
    let temporary = store
        .root()
        .join(format!(".activity-{:032x}.tmp", rand::random::<u128>()));
    let result: Result<()> = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&encoded)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, &target)?;
        File::open(store.root())?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result.context("saving authoritative activity selection")?;
    Ok(committed)
}

pub(crate) fn load_activity(store: &ActionContinuityStore) -> Result<ActivityRuntimeV1> {
    let _lock = store.reader_owner_transaction()?;
    let upgraded = store.root().join(RUNTIME_FILE).exists();
    if upgraded {
        verify_guard(store)?;
    }
    let path = store.root().join(if upgraded {
        RUNTIME_FILE
    } else if store.root().join(PREVIOUS_FILE).exists() {
        anyhow::ensure!(
            fs::read(store.root().join(LEGACY_FILE))? == DOWNGRADE_GUARD.as_bytes(),
            "older activity writer replaced the downgrade guard; retained for review"
        );
        PREVIOUS_FILE
    } else {
        LEGACY_FILE
    });
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActivityRuntimeV1::default());
        },
        Err(error) => return Err(error.into()),
    };
    if file.metadata()?.len() > MAX_RUNTIME_BYTES {
        return Err(anyhow!("activity selection exceeds size limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RUNTIME_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > MAX_RUNTIME_BYTES {
        return Err(anyhow!("activity selection exceeds size limit"));
    }
    let mut activity: ActivityRuntimeV1 = serde_json::from_slice(&bytes)
        .context("activity selection is corrupt; refusing legacy cursor recovery")?;
    validate_activity(&activity)?;
    if let Some(reader) = activity.return_reader.as_ref() {
        preview_reader(store, reader)?;
    }
    if let Some(reader) = activity.foreground_reader.clone() {
        let preview = preview_reader(store, &reader)?;
        let bookmark = preview.bookmark.context("reader bookmark missing")?;
        if !matches!(preview.session_status.as_str(), "active" | "summarized")
            || bookmark.disposition != ReaderDisposition::Active
        {
            // A session mutation may have survived an interrupted pointer write.
            // Quiet wins; restart cannot silently reactivate its old checkpoint.
            activity.foreground_reader = None;
            if activity.return_reader.is_none() {
                activity.return_reader = Some(reader);
            }
        } else if !preview
            .source_comparison
            .is_some_and(|source| source.retained_source_available)
        {
            return Err(anyhow!(
                "foreground retained source is unavailable; refusing legacy cursor recovery"
            ));
        }
    }
    Ok(activity)
}

fn verify_guard(store: &ActionContinuityStore) -> Result<()> {
    anyhow::ensure!(
        fs::read(store.root().join(LEGACY_FILE))? == DOWNGRADE_GUARD.as_bytes()
            && fs::read(store.root().join(PREVIOUS_FILE))? == PREVIOUS_GUARD.as_bytes(),
        "older activity writer replaced the downgrade guard; retained for review"
    );
    Ok(())
}

fn prepare_upgrade(store: &ActionContinuityStore) -> Result<()> {
    if store.root().join(RUNTIME_FILE).exists() {
        return verify_guard(store);
    }
    for (name, guard) in [
        (LEGACY_FILE, DOWNGRADE_GUARD),
        (PREVIOUS_FILE, PREVIOUS_GUARD),
    ] {
        let legacy = store.root().join(name);
        if legacy.exists() {
            let bytes = fs::read(&legacy)?;
            anyhow::ensure!(
                bytes != guard.as_bytes() || name == LEGACY_FILE,
                "interrupted activity migration; inspect retained legacy bytes"
            );
            if bytes != guard.as_bytes() {
                let _: ActivityRuntimeV1 = serde_json::from_slice(&bytes)?;
            }
            use sha2::{Digest, Sha256};
            let archive = store
                .root()
                .join(format!("activity_legacy_{:x}.json", Sha256::digest(&bytes)));
            if archive.exists() {
                anyhow::ensure!(
                    fs::read(&archive)? == bytes,
                    "legacy archive identity mismatch"
                );
            } else {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(archive)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
            }
        }
        let temporary = store.root().join(format!(
            ".activity-guard-{:032x}.tmp",
            rand::random::<u128>()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(guard.as_bytes())?;
        file.sync_all()?;
        fs::rename(temporary, legacy)?;
        File::open(store.root())?.sync_all()?;
    }
    Ok(())
}
