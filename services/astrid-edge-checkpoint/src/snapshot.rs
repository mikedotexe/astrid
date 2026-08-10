//! Immutable pre-switch state images and crash-safe rollback restoration.
//!
//! The image intentionally covers every persistent path writable by the core
//! or edge runtime. Operator hindsight is preserved across a rollback and is
//! expected to be made read-only to those services by the immutable unit
//! boundary. Socket/token state and service logs are explicitly ephemeral.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, chown};
use std::path::{Component, Path, PathBuf};

use astrid_edge_rescue_helper::fs_guard::{atomic_write, canonical_json, read_json, sha256};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{AUTHORITY, Error, Result, state_root, unix_millis, valid_identifier};

const MANIFEST_SCHEMA: &str = "astrid.edge_checkpoint.rollback_state.v2";
const RESTORE_SCHEMA: &str = "astrid.edge_checkpoint.restore_transaction.v1";
const MAX_FILES: usize = 200_000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_PRESERVED_FILES: usize = 100_000;
const QUIESCENCE_POLICY: &str = "exact_signed_runtime_stopped_transition_record";
const RETENTION_POLICY: &str = "paired_with_rollback_generation_no_independent_gc";
const MINIMUM_PRIOR_GENERATIONS: u32 = 3;
const MINIMUM_RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;

const COVERED_ROOTS: &[&str] = &[
    "state/home/default/edge",
    "state/home/default/.local/audit",
    "state/var",
    "state/keys",
];
// Operator hindsight lives at `state/operator`, outside every runtime-writable
// covered root. It therefore needs no detach/reattach exception: rollback
// cannot touch it at all.
const PRESERVED_PATHS: &[&str] = &[];
const EPHEMERAL_PATHS: &[&str] = &["state/run", "state/logs"];
const IMMUTABLE_PATHS: &[&str] = &["state/bin", "state/operator"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(
    clippy::struct_field_names,
    reason = "the wire field is manifest_sha256"
)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    created_at_unix_ms: u64,
    generation_id: String,
    state_device: u64,
    state_owner_uid: u32,
    state_owner_gid: u32,
    covered_roots: Vec<String>,
    preserved_paths: Vec<String>,
    ephemeral_paths: Vec<String>,
    immutable_paths: Vec<String>,
    quiescence_policy: String,
    quiescence_record_sha256: String,
    retention_policy: String,
    minimum_prior_generations: u32,
    minimum_retention_seconds: u64,
    rollback_semantics: String,
    files: Vec<Entry>,
    total_bytes: u64,
    content_inventory_sha256: String,
    authority: String,
    manifest_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    kind: EntryKind,
    size_bytes: u64,
    source_mode: u32,
    source_uid: u32,
    source_gid: u32,
    sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreJournal {
    schema: String,
    transaction_id: String,
    generation_id: String,
    snapshot_basename: String,
    snapshot_manifest_sha256: String,
    state_device: u64,
    staging_nonce: String,
    phase: String,
    restored_roots: Vec<String>,
    started_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    authority: String,
    journal_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBinding {
    pub generation_id: String,
    pub manifest_sha256: String,
}

pub fn create(
    workspace: &Path,
    output: &Path,
    generation_id: &str,
    quiescence_record_sha256: &str,
) -> Result<()> {
    create_inner(
        workspace,
        output,
        generation_id,
        quiescence_record_sha256,
        true,
    )
}

fn create_inner(
    workspace: &Path,
    output: &Path,
    generation_id: &str,
    quiescence_record_sha256: &str,
    require_root: bool,
) -> Result<()> {
    if !valid_identifier(generation_id) || !valid_hex64(quiescence_record_sha256) {
        return Err(Error::new("snapshot generation identity is invalid"));
    }
    require_output(output, false, require_root)?;
    let workspace = canonical_workspace(workspace)?;
    let root = state_root(&workspace)?;
    let root_metadata = safe_directory(&root)?;
    let runtime_owner = root_metadata.uid();
    let runtime_group = root_metadata.gid();
    if runtime_owner == 0 || workspace.metadata()?.uid() != runtime_owner {
        return Err(Error::new(
            "snapshot state and workspace must have one non-root runtime owner",
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| Error::new("snapshot output has no parent"))?;
    let name = output
        .file_name()
        .ok_or_else(|| Error::new("snapshot output has no basename"))?
        .to_string_lossy();
    let temporary = parent.join(format!(".{name}.{}.partial", std::process::id()));
    if temporary.exists() || temporary.is_symlink() {
        return Err(Error::new("snapshot temporary path collision"));
    }
    fs::create_dir(&temporary)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))?;
    let result = create_image(
        &root,
        &temporary,
        generation_id,
        quiescence_record_sha256,
        runtime_owner,
        runtime_group,
    );
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, output)?;
    File::open(parent)?.sync_all()?;
    verify_inner(output, generation_id, require_root).map(|_| ())
}

fn create_image(
    root: &Path,
    output: &Path,
    generation_id: &str,
    quiescence_record_sha256: &str,
    runtime_owner: u32,
    runtime_group: u32,
) -> Result<()> {
    let mut sources = Vec::new();
    for relative in COVERED_ROOTS {
        let relative = safe_relative(relative)?;
        let source = strip_state_prefix(root, &relative)?;
        collect(&source, root, runtime_owner, &mut sources)?;
    }
    sources.sort_by(|left, right| left.1.cmp(&right.1));
    sources.dedup_by(|left, right| left.1 == right.1);
    if sources.is_empty() || sources.len() > MAX_FILES {
        return Err(Error::new("snapshot inventory is empty or oversized"));
    }
    let mut entries = Vec::with_capacity(sources.len());
    let mut total = 0_u64;
    for (source, relative) in sources {
        let destination = output.join("fixture").join(&relative);
        let entry = copy_stable(&source, &destination, &relative, runtime_owner)?;
        total = total.saturating_add(entry.size_bytes);
        if total > MAX_TOTAL_BYTES {
            return Err(Error::new("snapshot total exceeds immutable byte bound"));
        }
        entries.push(entry);
    }
    let inventory_sha256 = inventory_digest(&entries)?;
    let mut manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        created_at_unix_ms: unix_millis(),
        generation_id: generation_id.to_owned(),
        state_device: safe_directory(root)?.dev(),
        state_owner_uid: runtime_owner,
        state_owner_gid: runtime_group,
        covered_roots: COVERED_ROOTS.iter().map(|item| (*item).to_owned()).collect(),
        preserved_paths: PRESERVED_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        ephemeral_paths: EPHEMERAL_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        immutable_paths: IMMUTABLE_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        quiescence_policy: QUIESCENCE_POLICY.to_owned(),
        quiescence_record_sha256: quiescence_record_sha256.to_owned(),
        retention_policy: RETENTION_POLICY.to_owned(),
        minimum_prior_generations: MINIMUM_PRIOR_GENERATIONS,
        minimum_retention_seconds: MINIMUM_RETENTION_SECONDS,
        rollback_semantics:
            "restore_exact_persistent_runtime_state_preserve_operator_hindsight_discard_probation_writes"
                .to_owned(),
        files: entries,
        total_bytes: total,
        content_inventory_sha256: inventory_sha256,
        authority: AUTHORITY.to_owned(),
        manifest_sha256: String::new(),
    };
    manifest.manifest_sha256 = manifest_digest(&manifest)?;
    write_new(
        &output.join("manifest.json"),
        &canonical_json(&manifest)?,
        0o400,
    )?;
    make_immutable(output)
}

pub fn verify(snapshot: &Path, generation_id: &str) -> Result<SnapshotBinding> {
    verify_inner(snapshot, generation_id, true)
}

fn verify_inner(
    snapshot: &Path,
    generation_id: &str,
    require_root: bool,
) -> Result<SnapshotBinding> {
    if !valid_identifier(generation_id) {
        return Err(Error::new("snapshot verification generation is invalid"));
    }
    require_output(snapshot, true, require_root)?;
    let manifest: Manifest = read_json(&snapshot.join("manifest.json"), 64 * 1024 * 1024)?;
    validate_manifest(&manifest, generation_id)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    let mut total = 0_u64;
    for entry in &manifest.files {
        let relative = safe_relative(&entry.path)?;
        let path = snapshot.join("fixture").join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != expected_uid
            || metadata.mode() & 0o222 != 0
        {
            return Err(Error::new("rollback state entry identity failed"));
        }
        match entry.kind {
            EntryKind::Directory => {
                if !metadata.is_dir() || entry.size_bytes != 0 || entry.sha256.is_some() {
                    return Err(Error::new("rollback state directory identity failed"));
                }
            },
            EntryKind::File => {
                if !metadata.is_file()
                    || metadata.nlink() != 1
                    || metadata.len() != entry.size_bytes
                    || entry.sha256.as_deref() != Some(file_hash(&path, MAX_FILE_BYTES)?.as_str())
                {
                    return Err(Error::new("rollback state file identity failed"));
                }
                total = total.saturating_add(entry.size_bytes);
            },
        }
    }
    if total != manifest.total_bytes || total > MAX_TOTAL_BYTES {
        return Err(Error::new("rollback state byte total failed"));
    }
    if collect_fixture_entries(&snapshot.join("fixture"))? != expected_fixture_entries(&manifest) {
        return Err(Error::new("rollback state contains unmanifested entries"));
    }
    Ok(SnapshotBinding {
        generation_id: manifest.generation_id,
        manifest_sha256: manifest.manifest_sha256,
    })
}

fn expected_fixture_entries(manifest: &Manifest) -> BTreeSet<String> {
    let mut expected = BTreeSet::new();
    for entry in &manifest.files {
        let path = Path::new(&entry.path);
        for ancestor in path
            .ancestors()
            .filter(|ancestor| !ancestor.as_os_str().is_empty())
        {
            expected.insert(ancestor.to_string_lossy().replace('\\', "/"));
        }
    }
    expected
}

pub fn restore(
    workspace: &Path,
    snapshot: &Path,
    generation_id: &str,
    transaction_id: &str,
) -> Result<()> {
    restore_inner(
        workspace,
        snapshot,
        generation_id,
        transaction_id,
        true,
        &mut |_| Ok(()),
    )
}

fn restore_inner<F>(
    workspace: &Path,
    snapshot: &Path,
    generation_id: &str,
    transaction_id: &str,
    require_root: bool,
    hook: &mut F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    if !valid_identifier(transaction_id) {
        return Err(Error::new("restore transaction identity is invalid"));
    }
    let binding = verify_inner(snapshot, generation_id, require_root)?;
    let root = restore_state_root(workspace)?;
    let root_metadata = safe_directory(&root)?;
    let manifest: Manifest = read_json(&snapshot.join("manifest.json"), 64 * 1024 * 1024)?;
    if root_metadata.dev() != manifest.state_device
        || root_metadata.uid() != manifest.state_owner_uid
        || root_metadata.gid() != manifest.state_owner_gid
    {
        return Err(Error::new(
            "live state device or owner differs from the sealed snapshot",
        ));
    }
    let snapshot_parent = snapshot
        .parent()
        .ok_or_else(|| Error::new("snapshot has no parent"))?;
    require_private_parent(snapshot_parent, require_root)?;
    let journal_path = snapshot_parent.join(format!("restore-{transaction_id}.json"));
    let snapshot_basename = snapshot
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| valid_identifier(value))
        .ok_or_else(|| Error::new("snapshot basename is invalid"))?;
    let mut journal = load_or_begin_journal(
        &journal_path,
        transaction_id,
        generation_id,
        snapshot_basename,
        &binding.manifest_sha256,
        manifest.state_device,
        require_root,
    )?;
    validate_journal_binding(
        &journal,
        transaction_id,
        generation_id,
        snapshot_basename,
        &binding.manifest_sha256,
        manifest.state_device,
    )?;
    if journal.phase == "completed" {
        verify_live_state(&root, &manifest)?;
        cleanup_transaction_paths(&root, &manifest, &journal)?;
        return Ok(());
    }
    hook("journal_bound")?;
    prepare_staging(&root, snapshot, &manifest, &journal)?;
    update_journal(&journal_path, &mut journal, "prepared", require_root)?;
    hook("prepared")?;
    preserve_operator_state(&root, &manifest, &journal)?;
    update_journal(
        &journal_path,
        &mut journal,
        "preserved_paths_detached",
        require_root,
    )?;
    hook("preserved_paths_detached")?;
    for covered in &manifest.covered_roots {
        restore_one_root(&root, snapshot, &manifest, &journal, covered)?;
        if !journal.restored_roots.contains(covered) {
            journal.restored_roots.push(covered.clone());
        }
        update_journal(&journal_path, &mut journal, "restoring", require_root)?;
        hook(&format!("restored:{covered}"))?;
    }
    reattach_operator_state(&root, &manifest, &journal)?;
    update_journal(
        &journal_path,
        &mut journal,
        "preserved_paths_reattached",
        require_root,
    )?;
    hook("preserved_paths_reattached")?;
    verify_live_state(&root, &manifest)?;
    update_journal(&journal_path, &mut journal, "completed", require_root)?;
    hook("completed")?;
    cleanup_transaction_paths(&root, &manifest, &journal)?;
    Ok(())
}

fn validate_manifest(manifest: &Manifest, generation_id: &str) -> Result<()> {
    if manifest.schema != MANIFEST_SCHEMA
        || manifest.generation_id != generation_id
        || manifest.created_at_unix_ms == 0
        || manifest.state_device == 0
        || manifest.state_owner_uid == 0
        || manifest.covered_roots != COVERED_ROOTS
        || manifest.preserved_paths != PRESERVED_PATHS
        || manifest.ephemeral_paths != EPHEMERAL_PATHS
        || manifest.immutable_paths != IMMUTABLE_PATHS
        || manifest.quiescence_policy != QUIESCENCE_POLICY
        || !valid_hex64(&manifest.quiescence_record_sha256)
        || manifest.retention_policy != RETENTION_POLICY
        || manifest.minimum_prior_generations != MINIMUM_PRIOR_GENERATIONS
        || manifest.minimum_retention_seconds != MINIMUM_RETENTION_SECONDS
        || manifest.rollback_semantics
            != "restore_exact_persistent_runtime_state_preserve_operator_hindsight_discard_probation_writes"
        || manifest.authority != AUTHORITY
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_FILES
        || manifest.manifest_sha256 != manifest_digest(manifest)?
        || manifest.content_inventory_sha256 != inventory_digest(&manifest.files)?
    {
        return Err(Error::new("rollback state manifest identity failed"));
    }
    let roots = manifest.covered_roots.iter().collect::<BTreeSet<_>>();
    if roots.len() != manifest.covered_roots.len()
        || manifest.files.iter().any(|entry| {
            safe_relative(&entry.path).is_err()
                || !roots
                    .iter()
                    .any(|root| path_is_within(&entry.path, root.as_str()))
                || entry.source_uid != manifest.state_owner_uid
                || entry.source_gid != manifest.state_owner_gid
                || entry.source_mode & 0o7000 != 0
                || (entry.kind == EntryKind::Directory
                    && (entry.size_bytes != 0 || entry.sha256.is_some()))
                || (entry.kind == EntryKind::File
                    && (entry.size_bytes > MAX_FILE_BYTES
                        || entry
                            .sha256
                            .as_deref()
                            .is_none_or(|value| !valid_hex64(value))))
        })
    {
        return Err(Error::new("rollback state manifest entries are invalid"));
    }
    for root in &manifest.covered_roots {
        if !manifest
            .files
            .iter()
            .any(|entry| entry.path == *root && entry.kind == EntryKind::Directory)
        {
            return Err(Error::new("rollback state lacks a covered root"));
        }
    }
    Ok(())
}

fn collect(
    source: &Path,
    state_root: &Path,
    owner_uid: u32,
    output: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    validate_source_metadata(&metadata, owner_uid)?;
    let relative = PathBuf::from("state").join(
        source
            .strip_prefix(state_root)
            .map_err(|_| Error::new("snapshot source path escaped"))?,
    );
    if PRESERVED_PATHS
        .iter()
        .any(|preserved| path_is_within(&relative.to_string_lossy(), preserved))
    {
        return Ok(());
    }
    output.push((source.to_path_buf(), relative));
    if metadata.is_dir() {
        let mut children = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            collect(&child.path(), state_root, owner_uid, output)?;
            if output.len() > MAX_FILES {
                return Err(Error::new(
                    "snapshot file inventory exceeds immutable bound",
                ));
            }
        }
    }
    Ok(())
}

fn validate_source_metadata(metadata: &fs::Metadata, owner_uid: u32) -> Result<()> {
    if metadata.file_type().is_symlink()
        || (!metadata.is_dir() && !metadata.is_file())
        || (metadata.is_file() && (metadata.nlink() != 1 || metadata.len() > MAX_FILE_BYTES))
        || metadata.uid() != owner_uid
        || metadata.mode() & 0o7000 != 0
    {
        return Err(Error::new(
            "snapshot source contains unsafe ownership, links, mode, or type",
        ));
    }
    Ok(())
}

fn copy_stable(
    source: &Path,
    destination: &Path,
    relative: &Path,
    owner_uid: u32,
) -> Result<Entry> {
    let before = fs::symlink_metadata(source)?;
    validate_source_metadata(&before, owner_uid)?;
    if before.is_dir() {
        fs::create_dir_all(destination)?;
        return Ok(Entry {
            path: relative.to_string_lossy().replace('\\', "/"),
            kind: EntryKind::Directory,
            size_bytes: 0,
            source_mode: before.mode() & 0o777,
            source_uid: before.uid(),
            source_gid: before.gid(),
            sha256: None,
        });
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut input = options.open(source)?;
    let opened = input.metadata()?;
    if file_identity(&before) != file_identity(&opened) {
        return Err(Error::new("snapshot source changed before copy"));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o400);
    let mut output = options.open(destination)?;
    let mut remaining = before.len();
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = input.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(Error::new("snapshot source ended early"));
        }
        output.write_all(&buffer[..read])?;
        hash.update(&buffer[..read]);
        remaining = remaining.saturating_sub(u64::try_from(read).unwrap_or(u64::MAX));
    }
    output.sync_all()?;
    if file_identity(&opened) != file_identity(&input.metadata()?)
        || file_identity(&before) != file_identity(&fs::symlink_metadata(source)?)
    {
        return Err(Error::new("snapshot source changed during copy"));
    }
    Ok(Entry {
        path: relative.to_string_lossy().replace('\\', "/"),
        kind: EntryKind::File,
        size_bytes: before.len(),
        source_mode: before.mode() & 0o777,
        source_uid: before.uid(),
        source_gid: before.gid(),
        sha256: Some(format!("{:x}", hash.finalize())),
    })
}

fn prepare_staging(
    root: &Path,
    snapshot: &Path,
    manifest: &Manifest,
    journal: &RestoreJournal,
) -> Result<()> {
    for covered in &manifest.covered_roots {
        let staging = staging_path(root, covered, journal, "new")?;
        if staging.exists() || staging.is_symlink() {
            if tree_matches_root(&staging, snapshot, manifest, covered)? {
                continue;
            }
            // The basename contains the random nonce sealed into this exact
            // root-owned restore journal. Mutable runtime is stopped before
            // the journal exists, so an incomplete regular directory here is
            // an interrupted materialization, not an attacker-selected path.
            // Symlinks and non-directories still fail closed in
            // `remove_exact_tree`.
            remove_exact_tree(root, &staging)?;
        }
        materialize_root(&staging, snapshot, manifest, covered)?;
        if !tree_matches_root(&staging, snapshot, manifest, covered)? {
            return Err(Error::new("restore staging root verification failed"));
        }
        File::open(root)?.sync_all()?;
    }
    Ok(())
}

fn materialize_root(
    staging: &Path,
    snapshot: &Path,
    manifest: &Manifest,
    covered: &str,
) -> Result<()> {
    let root_entry = manifest
        .files
        .iter()
        .find(|entry| entry.path == covered && entry.kind == EntryKind::Directory)
        .ok_or_else(|| Error::new("snapshot covered root entry is absent"))?;
    let mut options = fs::DirBuilder::new();
    options.mode(0o700);
    options.create(staging)?;
    let mut entries = manifest
        .files
        .iter()
        .filter(|entry| entry.path != covered && path_is_within(&entry.path, covered))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path.matches('/').count());
    for entry in &entries {
        let suffix = Path::new(&entry.path)
            .strip_prefix(covered)
            .map_err(|_| Error::new("snapshot entry escaped covered root"))?;
        let destination = staging.join(suffix);
        match entry.kind {
            EntryKind::Directory => {
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700);
                builder.create(&destination)?;
            },
            EntryKind::File => restore_file(snapshot, &destination, entry)?,
        }
    }
    let mut directories = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Directory)
        .copied()
        .collect::<Vec<_>>();
    directories.sort_by_key(|entry| std::cmp::Reverse(entry.path.matches('/').count()));
    for entry in directories {
        let suffix = Path::new(&entry.path)
            .strip_prefix(covered)
            .map_err(|_| Error::new("snapshot directory escaped covered root"))?;
        apply_metadata(&staging.join(suffix), entry)?;
    }
    apply_metadata(staging, root_entry)?;
    File::open(staging)?.sync_all()?;
    Ok(())
}

fn restore_file(snapshot: &Path, destination: &Path, entry: &Entry) -> Result<()> {
    let source = snapshot.join("fixture").join(safe_relative(&entry.path)?);
    let mut input_options = OpenOptions::new();
    input_options
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut input = input_options.open(&source)?;
    let source_metadata = input.metadata()?;
    if !source_metadata.is_file()
        || source_metadata.nlink() != 1
        || source_metadata.len() != entry.size_bytes
    {
        return Err(Error::new("snapshot restore source identity failed"));
    }
    let mut output_options = OpenOptions::new();
    output_options
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut output = output_options.open(destination)?;
    let mut hash = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        hash.update(&buffer[..read]);
        copied = copied.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    output.sync_all()?;
    if copied != entry.size_bytes
        || entry.sha256.as_deref() != Some(format!("{:x}", hash.finalize()).as_str())
    {
        return Err(Error::new("snapshot restore file digest failed"));
    }
    apply_metadata(destination, entry)
}

fn apply_metadata(path: &Path, entry: &Entry) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(entry.source_mode))?;
    chown(path, Some(entry.source_uid), Some(entry.source_gid))?;
    Ok(())
}

fn preserve_operator_state(
    root: &Path,
    manifest: &Manifest,
    journal: &RestoreJournal,
) -> Result<()> {
    for preserved in &manifest.preserved_paths {
        let live = strip_state_prefix(root, Path::new(preserved))?;
        let detached = preserve_path(root, preserved, journal);
        if detached.exists() || detached.is_symlink() {
            validate_preserved_tree(
                root,
                &state_relative(root, &detached)?,
                manifest.state_owner_uid,
            )?;
            continue;
        }
        if live.exists() || live.is_symlink() {
            validate_preserved_tree(root, Path::new(preserved), manifest.state_owner_uid)?;
            fs::rename(&live, &detached)?;
            sync_parent(&live)?;
            File::open(root)?.sync_all()?;
            continue;
        }
        for covered in &manifest.covered_roots {
            if path_is_within(preserved, covered) {
                let old = staging_path(root, covered, journal, "old")?;
                let suffix = Path::new(preserved)
                    .strip_prefix(covered)
                    .map_err(|_| Error::new("preserved path escaped covered root"))?;
                let old_preserved = old.join(suffix);
                if old_preserved.exists() || old_preserved.is_symlink() {
                    validate_tree_path(&old_preserved, manifest.state_owner_uid)?;
                    fs::rename(&old_preserved, &detached)?;
                    sync_parent(&old_preserved)?;
                    File::open(root)?.sync_all()?;
                    break;
                }
            }
        }
        if !detached.exists() {
            return Err(Error::new(
                "required operator state disappeared during restore",
            ));
        }
    }
    Ok(())
}

fn restore_one_root(
    root: &Path,
    snapshot: &Path,
    manifest: &Manifest,
    journal: &RestoreJournal,
    covered: &str,
) -> Result<()> {
    let live = strip_state_prefix(root, Path::new(covered))?;
    let staged = staging_path(root, covered, journal, "new")?;
    let old = staging_path(root, covered, journal, "old")?;
    if live.exists() && tree_matches_root(&live, snapshot, manifest, covered)? {
        return Ok(());
    }
    if old.exists() || old.is_symlink() {
        if live.exists() || live.is_symlink() {
            return Err(Error::new(
                "restore found both an untrusted live root and prior backup",
            ));
        }
    } else if live.exists() || live.is_symlink() {
        fs::rename(&live, &old)?;
        sync_parent(&live)?;
    }
    if !staged.exists() || !tree_matches_root(&staged, snapshot, manifest, covered)? {
        return Err(Error::new("restore staged root is absent or invalid"));
    }
    if let Some(parent) = live.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&staged, &live)?;
    sync_parent(&live)?;
    if !tree_matches_root(&live, snapshot, manifest, covered)? {
        return Err(Error::new("restored live root failed exact verification"));
    }
    Ok(())
}

fn reattach_operator_state(
    root: &Path,
    manifest: &Manifest,
    journal: &RestoreJournal,
) -> Result<()> {
    for preserved in &manifest.preserved_paths {
        let live = strip_state_prefix(root, Path::new(preserved))?;
        if live.exists() || live.is_symlink() {
            validate_tree_path(&live, manifest.state_owner_uid)?;
            continue;
        }
        let detached = preserve_path(root, preserved, journal);
        if !detached.exists() || detached.is_symlink() {
            return Err(Error::new("detached operator state is absent or unsafe"));
        }
        if let Some(parent) = live.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&detached, &live)?;
        sync_parent(&live)?;
        validate_tree_path(&live, manifest.state_owner_uid)?;
    }
    Ok(())
}

fn verify_live_state(root: &Path, manifest: &Manifest) -> Result<()> {
    for covered in &manifest.covered_roots {
        let live = strip_state_prefix(root, Path::new(covered))?;
        if !tree_matches_manifest_root(&live, manifest, covered)? {
            return Err(Error::new(
                "live restored state differs from sealed snapshot",
            ));
        }
    }
    for preserved in &manifest.preserved_paths {
        validate_preserved_tree(root, Path::new(preserved), manifest.state_owner_uid)?;
    }
    Ok(())
}

fn tree_matches_root(
    candidate: &Path,
    snapshot: &Path,
    manifest: &Manifest,
    covered: &str,
) -> Result<bool> {
    if !tree_matches_manifest_root(candidate, manifest, covered)? {
        return Ok(false);
    }
    for entry in manifest
        .files
        .iter()
        .filter(|entry| path_is_within(&entry.path, covered))
    {
        if entry.kind != EntryKind::File {
            continue;
        }
        let suffix = Path::new(&entry.path)
            .strip_prefix(covered)
            .map_err(|_| Error::new("snapshot compare entry escaped"))?;
        let candidate_file = candidate.join(suffix);
        let fixture_file = snapshot.join("fixture").join(&entry.path);
        if file_hash(&candidate_file, MAX_FILE_BYTES)? != file_hash(&fixture_file, MAX_FILE_BYTES)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn tree_matches_manifest_root(
    candidate: &Path,
    manifest: &Manifest,
    covered: &str,
) -> Result<bool> {
    let metadata = match fs::symlink_metadata(candidate) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let mut actual = BTreeMap::new();
    collect_live_entries(candidate, candidate, covered, &mut actual)?;
    let expected = manifest
        .files
        .iter()
        .filter(|entry| path_is_within(&entry.path, covered))
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (path, metadata) in actual {
        let Some(entry) = expected.get(&path) else {
            return Ok(false);
        };
        if metadata.uid() != entry.source_uid
            || metadata.gid() != entry.source_gid
            || metadata.mode() & 0o777 != entry.source_mode
            || metadata.file_type().is_symlink()
        {
            return Ok(false);
        }
        match entry.kind {
            EntryKind::Directory if !metadata.is_dir() => return Ok(false),
            EntryKind::File
                if !metadata.is_file()
                    || metadata.nlink() != 1
                    || metadata.len() != entry.size_bytes
                    || entry.sha256.as_deref()
                        != Some(
                            file_hash(
                                &candidate.join(relative_suffix(&path, covered)?),
                                MAX_FILE_BYTES,
                            )?
                            .as_str(),
                        ) =>
            {
                return Ok(false);
            },
            _ => {},
        }
    }
    Ok(true)
}

fn collect_live_entries(
    base: &Path,
    path: &Path,
    covered: &str,
    output: &mut BTreeMap<String, fs::Metadata>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err(Error::new("restored state contains a link or special file"));
    }
    let suffix = path
        .strip_prefix(base)
        .map_err(|_| Error::new("restored state traversal escaped"))?;
    let relative = if suffix.as_os_str().is_empty() {
        covered.to_owned()
    } else {
        format!("{covered}/{}", suffix.to_string_lossy().replace('\\', "/"))
    };
    if PRESERVED_PATHS
        .iter()
        .any(|preserved| path_is_within(&relative, preserved))
    {
        return Ok(());
    }
    output.insert(relative, metadata.clone());
    if metadata.is_dir() {
        let mut children = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            collect_live_entries(base, &child.path(), covered, output)?;
            if output.len() > MAX_FILES {
                return Err(Error::new("restored state inventory exceeds bound"));
            }
        }
    }
    Ok(())
}

fn relative_suffix(path: &str, covered: &str) -> Result<PathBuf> {
    Path::new(path)
        .strip_prefix(covered)
        .map(Path::to_path_buf)
        .map_err(|_| Error::new("live state entry escaped covered root"))
}

fn load_or_begin_journal(
    path: &Path,
    transaction_id: &str,
    generation_id: &str,
    snapshot_basename: &str,
    snapshot_manifest_sha256: &str,
    state_device: u64,
    require_root: bool,
) -> Result<RestoreJournal> {
    if path.exists() || path.is_symlink() {
        let journal: RestoreJournal = read_json(path, 64 * 1024)?;
        validate_journal(&journal)?;
        require_journal_file(path, require_root)?;
        return Ok(journal);
    }
    let mut journal = RestoreJournal {
        schema: RESTORE_SCHEMA.to_owned(),
        transaction_id: transaction_id.to_owned(),
        generation_id: generation_id.to_owned(),
        snapshot_basename: snapshot_basename.to_owned(),
        snapshot_manifest_sha256: snapshot_manifest_sha256.to_owned(),
        state_device,
        staging_nonce: random_nonce()?,
        phase: "planned".to_owned(),
        restored_roots: Vec::new(),
        started_at_unix_ms: unix_millis(),
        updated_at_unix_ms: unix_millis(),
        authority: AUTHORITY.to_owned(),
        journal_sha256: String::new(),
    };
    journal.journal_sha256 = journal_digest(&journal)?;
    atomic_write(path, &canonical_json(&journal)?, 0o600, false)?;
    require_journal_file(path, require_root)?;
    Ok(journal)
}

fn update_journal(
    path: &Path,
    journal: &mut RestoreJournal,
    phase: &str,
    require_root: bool,
) -> Result<()> {
    if !matches!(
        phase,
        "prepared"
            | "preserved_paths_detached"
            | "restoring"
            | "preserved_paths_reattached"
            | "completed"
    ) {
        return Err(Error::new("restore journal phase is invalid"));
    }
    phase.clone_into(&mut journal.phase);
    journal.updated_at_unix_ms = unix_millis();
    journal.journal_sha256 = journal_digest(journal)?;
    atomic_write(path, &canonical_json(journal)?, 0o600, true)?;
    require_journal_file(path, require_root)
}

fn validate_journal(journal: &RestoreJournal) -> Result<()> {
    if journal.schema != RESTORE_SCHEMA
        || !valid_identifier(&journal.transaction_id)
        || !valid_identifier(&journal.generation_id)
        || !valid_identifier(&journal.snapshot_basename)
        || !valid_hex64(&journal.snapshot_manifest_sha256)
        || journal.state_device == 0
        || !valid_hex64(&journal.staging_nonce)
        || !matches!(
            journal.phase.as_str(),
            "planned"
                | "prepared"
                | "preserved_paths_detached"
                | "restoring"
                | "preserved_paths_reattached"
                | "completed"
        )
        || journal.restored_roots.len() > COVERED_ROOTS.len()
        || journal
            .restored_roots
            .iter()
            .any(|root| !COVERED_ROOTS.contains(&root.as_str()))
        || journal.started_at_unix_ms == 0
        || journal.updated_at_unix_ms < journal.started_at_unix_ms
        || journal.authority != AUTHORITY
        || journal.journal_sha256 != journal_digest(journal)?
    {
        return Err(Error::new("restore transaction journal failed validation"));
    }
    Ok(())
}

fn validate_journal_binding(
    journal: &RestoreJournal,
    transaction_id: &str,
    generation_id: &str,
    snapshot_basename: &str,
    manifest_sha256: &str,
    state_device: u64,
) -> Result<()> {
    validate_journal(journal)?;
    if journal.transaction_id != transaction_id
        || journal.generation_id != generation_id
        || journal.snapshot_basename != snapshot_basename
        || journal.snapshot_manifest_sha256 != manifest_sha256
        || journal.state_device != state_device
    {
        return Err(Error::new(
            "restore journal differs from exact snapshot/generation binding",
        ));
    }
    Ok(())
}

fn require_journal_file(path: &Path, require_root: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("restore journal identity failed"));
    }
    Ok(())
}

fn cleanup_transaction_paths(
    root: &Path,
    manifest: &Manifest,
    journal: &RestoreJournal,
) -> Result<()> {
    for covered in &manifest.covered_roots {
        for side in ["new", "old"] {
            let path = staging_path(root, covered, journal, side)?;
            if path.exists() || path.is_symlink() {
                remove_exact_tree(root, &path)?;
            }
        }
    }
    for preserved in &manifest.preserved_paths {
        let path = preserve_path(root, preserved, journal);
        if path.exists() || path.is_symlink() {
            return Err(Error::new(
                "restore completed with detached operator evidence",
            ));
        }
    }
    File::open(root)?.sync_all()?;
    Ok(())
}

fn remove_exact_tree(root: &Path, path: &Path) -> Result<()> {
    if path.parent() != Some(root) || !path.starts_with(root) {
        return Err(Error::new("restore cleanup target escaped state root"));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(
            "restore cleanup target is not an exact directory",
        ));
    }
    fs::remove_dir_all(path)?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn staging_path(
    root: &Path,
    covered: &str,
    journal: &RestoreJournal,
    side: &str,
) -> Result<PathBuf> {
    if !matches!(side, "new" | "old") {
        return Err(Error::new("restore staging side is invalid"));
    }
    let label = sha256(covered.as_bytes());
    Ok(root.join(format!(
        ".astrid-restore-{}-{}-{side}",
        &journal.staging_nonce[..24],
        &label[..16]
    )))
}

fn preserve_path(root: &Path, preserved: &str, journal: &RestoreJournal) -> PathBuf {
    let label = sha256(preserved.as_bytes());
    root.join(format!(
        ".astrid-restore-{}-{}-preserved",
        &journal.staging_nonce[..24],
        &label[..16]
    ))
}

fn validate_preserved_tree(root: &Path, relative: &Path, owner_uid: u32) -> Result<()> {
    let path = strip_state_prefix(root, relative)?;
    validate_tree_path(&path, owner_uid)
}

fn validate_tree_path(path: &Path, owner_uid: u32) -> Result<()> {
    fn walk(path: &Path, owner_uid: u32, count: &mut usize) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != owner_uid
            || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
        {
            return Err(Error::new("preserved operator tree identity failed"));
        }
        *count = count.saturating_add(1);
        if *count > MAX_PRESERVED_FILES {
            return Err(Error::new("preserved operator tree exceeds bound"));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(&entry?.path(), owner_uid, count)?;
            }
        }
        Ok(())
    }
    let mut count = 0;
    walk(path, owner_uid, &mut count)
}

fn collect_fixture_entries(root: &Path) -> Result<BTreeSet<String>> {
    fn walk(root: &Path, path: &Path, output: &mut BTreeSet<String>) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && (!metadata.is_file() || metadata.nlink() != 1))
        {
            return Err(Error::new(
                "rollback state contains a special or linked entry",
            ));
        }
        if path != root {
            output.insert(
                path.strip_prefix(root)
                    .map_err(|_| Error::new("rollback state path escaped"))?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(root, &entry?.path(), output)?;
            }
        }
        Ok(())
    }
    let mut output = BTreeSet::new();
    walk(root, root, &mut output)?;
    Ok(output)
}

fn make_immutable(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            make_immutable(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o500))?;
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
        }
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o500))?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn require_output(path: &Path, must_exist: bool, require_root: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("snapshot path is not absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("snapshot path has no parent"))?;
    require_private_parent(parent, require_root)?;
    if (must_exist && (!path.is_dir() || path.is_symlink()))
        || (!must_exist && (path.exists() || path.is_symlink()))
    {
        return Err(Error::new("snapshot output state is invalid"));
    }
    Ok(())
}

fn require_private_parent(path: &Path, require_root: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error::new("snapshot parent is not private immutable state"));
    }
    Ok(())
}

fn canonical_workspace(workspace: &Path) -> Result<PathBuf> {
    let canonical = workspace.canonicalize()?;
    let metadata = safe_directory(&canonical)?;
    if metadata.uid() == 0 {
        return Err(Error::new("workspace may not be root-owned"));
    }
    Ok(canonical)
}

fn restore_state_root(workspace: &Path) -> Result<PathBuf> {
    let suffix = Path::new("home/default/edge");
    if !workspace.is_absolute()
        || !workspace.ends_with(suffix)
        || workspace.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::new("restore workspace layout is invalid"));
    }
    let lexical_root = workspace
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error::new("restore state root cannot be derived"))?;
    let root = lexical_root
        .canonicalize()
        .map_err(|error| Error::new(format!("cannot resolve restore state root: {error}")))?;
    if root == Path::new("/") || root.as_os_str().is_empty() {
        return Err(Error::new("restore state root is unsafe"));
    }
    safe_directory(&root)?;
    Ok(root)
}

fn safe_directory(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("state path is not a non-symlink directory"));
    }
    Ok(metadata)
}

fn safe_relative(value: impl AsRef<Path>) -> Result<PathBuf> {
    let path = value.as_ref();
    let string = path.to_string_lossy();
    if string.is_empty()
        || string.len() > 1_024
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(Error::new("snapshot manifest path is unsafe"));
    }
    Ok(path.to_path_buf())
}

fn strip_state_prefix(root: &Path, relative: &Path) -> Result<PathBuf> {
    let relative = safe_relative(relative)?;
    let stripped = relative
        .strip_prefix("state")
        .map_err(|_| Error::new("snapshot path lacks state prefix"))?;
    if stripped.as_os_str().is_empty() {
        return Err(Error::new("snapshot path resolves to the whole state root"));
    }
    Ok(root.join(stripped))
}

fn state_relative(root: &Path, path: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from("state").join(
        path.strip_prefix(root)
            .map_err(|_| Error::new("state path escaped root"))?,
    ))
}

fn path_is_within(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn file_hash(path: &Path, maximum: u64) -> Result<String> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > maximum
    {
        return Err(Error::new("snapshot file identity or bound failed"));
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    let mut hash = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        copied = copied.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    if copied != before.len()
        || file_identity(&before) != file_identity(&opened)
        || file_identity(&opened) != file_identity(&file.metadata()?)
        || file_identity(&before) != file_identity(&fs::symlink_metadata(path)?)
    {
        return Err(Error::new("snapshot file changed while hashed"));
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn inventory_digest(entries: &[Entry]) -> Result<String> {
    Ok(sha256(&canonical_json(&entries.to_vec())?))
}

fn manifest_digest(manifest: &Manifest) -> Result<String> {
    let mut value = serde_json::to_value(manifest)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("snapshot manifest serialization failed"))?
        .remove("manifest_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

fn journal_digest(journal: &RestoreJournal) -> Result<String> {
    let mut value = serde_json::to_value(journal)?;
    value
        .as_object_mut()
        .ok_or_else(|| Error::new("restore journal serialization failed"))?
        .remove("journal_sha256");
    Ok(sha256(&canonical_json(&value)?))
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn file_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn valid_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn random_nonce() -> Result<String> {
    let mut entropy = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    Ok(sha256(&entropy))
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("state path has no parent"))?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Entry, EntryKind, Manifest, RestoreJournal, create_inner, inventory_digest,
        manifest_digest, restore_inner, safe_relative, staging_path, verify_inner,
    };
    use crate::Error;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let state = root.join("state");
        let workspace = state.join("home/default/edge");
        for path in [
            workspace.join("research/nested"),
            state.join("home/default/.local/audit"),
            state.join("var"),
            state.join("keys"),
            state.join("run"),
            state.join("logs"),
            state.join("bin"),
            state.join("operator"),
        ] {
            fs::create_dir_all(path).unwrap();
        }
        fs::write(workspace.join("journal.md"), b"baseline journal").unwrap();
        fs::write(
            workspace.join("research/nested/evidence.md"),
            b"baseline evidence",
        )
        .unwrap();
        fs::write(state.join("home/default/.local/audit/audit.db"), b"audit-1").unwrap();
        fs::write(state.join("var/state.db"), b"db-1").unwrap();
        fs::write(state.join("keys/runtime.key"), b"key-1").unwrap();
        fs::write(state.join("run/system.token"), b"ephemeral-1").unwrap();
        fs::write(state.join("logs/service.log"), b"log-1").unwrap();
        fs::write(state.join("operator/hindsight.db"), b"hindsight-1").unwrap();
        let snapshots = root.join("snapshots");
        fs::create_dir(&snapshots).unwrap();
        fs::set_permissions(&snapshots, fs::Permissions::from_mode(0o700)).unwrap();
        (workspace, snapshots)
    }

    #[test]
    fn manifest_hashes_bind_paths_kinds_and_content() {
        let entry = Entry {
            path: "state/var/state.db".into(),
            kind: EntryKind::File,
            size_bytes: 1,
            source_mode: 0o600,
            source_uid: 42,
            source_gid: 42,
            sha256: Some("a".repeat(64)),
        };
        let first = inventory_digest(std::slice::from_ref(&entry)).unwrap();
        let mut changed = entry.clone();
        changed.path = "state/var/other".into();
        assert_ne!(first, inventory_digest(&[changed]).unwrap());
        let manifest = Manifest {
            schema: "astrid.edge_checkpoint.rollback_state.v2".into(),
            created_at_unix_ms: 1,
            generation_id: "gen-a".into(),
            state_device: 1,
            state_owner_uid: 42,
            state_owner_gid: 42,
            covered_roots: super::COVERED_ROOTS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            preserved_paths: super::PRESERVED_PATHS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            ephemeral_paths: super::EPHEMERAL_PATHS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            immutable_paths: super::IMMUTABLE_PATHS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            quiescence_policy: super::QUIESCENCE_POLICY.into(),
            quiescence_record_sha256: "b".repeat(64),
            retention_policy: super::RETENTION_POLICY.into(),
            minimum_prior_generations: super::MINIMUM_PRIOR_GENERATIONS,
            minimum_retention_seconds: super::MINIMUM_RETENTION_SECONDS,
            rollback_semantics:
                "restore_exact_persistent_runtime_state_preserve_operator_hindsight_discard_probation_writes"
                    .into(),
            files: vec![entry],
            total_bytes: 1,
            content_inventory_sha256: first,
            authority: super::AUTHORITY.into(),
            manifest_sha256: String::new(),
        };
        assert_eq!(manifest_digest(&manifest).unwrap().len(), 64);
    }

    #[test]
    fn snapshot_paths_are_plain_relative_components() {
        assert!(safe_relative("state/home/default/edge/value.json").is_ok());
        assert!(safe_relative("../outside").is_err());
        assert!(safe_relative("/absolute").is_err());
    }

    #[test]
    fn rollback_restores_persistent_state_and_preserves_operator_and_ephemeral_state() {
        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
        verify_inner(&snapshot, "gen-old", false).unwrap();

        fs::write(workspace.join("journal.md"), b"candidate corruption").unwrap();
        fs::write(workspace.join("new-candidate-file"), b"candidate").unwrap();
        fs::remove_dir_all(workspace.join("research")).unwrap();
        fs::write(
            temp.path().join("state/home/default/.local/audit/audit.db"),
            b"candidate audit corruption",
        )
        .unwrap();
        fs::remove_file(temp.path().join("state/var/state.db")).unwrap();
        fs::write(temp.path().join("state/keys/runtime.key"), b"bad key").unwrap();
        fs::write(
            temp.path().join("state/operator/hindsight.db"),
            b"hindsight-2",
        )
        .unwrap();
        fs::write(temp.path().join("state/run/system.token"), b"ephemeral-2").unwrap();
        fs::write(temp.path().join("state/logs/service.log"), b"log-2").unwrap();

        restore_inner(
            &workspace,
            &snapshot,
            "gen-old",
            "restore-a",
            false,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(workspace.join("journal.md")).unwrap(),
            b"baseline journal"
        );
        assert!(!workspace.join("new-candidate-file").exists());
        assert_eq!(
            fs::read(workspace.join("research/nested/evidence.md")).unwrap(),
            b"baseline evidence"
        );
        assert_eq!(
            fs::read(temp.path().join("state/home/default/.local/audit/audit.db")).unwrap(),
            b"audit-1"
        );
        assert_eq!(
            fs::read(temp.path().join("state/var/state.db")).unwrap(),
            b"db-1"
        );
        assert_eq!(
            fs::read(temp.path().join("state/keys/runtime.key")).unwrap(),
            b"key-1"
        );
        assert_eq!(
            fs::read(temp.path().join("state/run/system.token")).unwrap(),
            b"ephemeral-2"
        );
        assert_eq!(
            fs::read(temp.path().join("state/logs/service.log")).unwrap(),
            b"log-2"
        );
        assert_eq!(
            fs::read(temp.path().join("state/operator/hindsight.db")).unwrap(),
            b"hindsight-2"
        );
    }

    #[test]
    fn every_restore_phase_is_replayable_and_idempotent() {
        for (index, phase) in [
            "journal_bound",
            "prepared",
            "preserved_paths_detached",
            "restored:state/home/default/edge",
            "restored:state/home/default/.local/audit",
            "restored:state/var",
            "restored:state/keys",
            "preserved_paths_reattached",
            "completed",
        ]
        .iter()
        .enumerate()
        {
            let temp = tempfile::tempdir().unwrap();
            let (workspace, snapshots) = fixture(temp.path());
            let snapshot = snapshots.join("baseline");
            create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
            fs::write(workspace.join("journal.md"), b"candidate").unwrap();
            let stop = (*phase).to_owned();
            let result = restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                &format!("crash-{index}"),
                false,
                &mut |current| {
                    if current == stop {
                        return Err(Error::new("simulated power loss"));
                    }
                    Ok(())
                },
            );
            assert!(result.is_err());
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                &format!("crash-{index}"),
                false,
                &mut |_| Ok(()),
            )
            .unwrap();
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                &format!("crash-{index}"),
                false,
                &mut |_| Ok(()),
            )
            .unwrap();
            assert_eq!(
                fs::read(workspace.join("journal.md")).unwrap(),
                b"baseline journal"
            );
        }
    }

    #[test]
    fn snapshot_and_restore_reject_links_and_binding_replay() {
        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        std::os::unix::fs::symlink(workspace.join("journal.md"), workspace.join("bad-link"))
            .unwrap();
        assert!(create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).is_err());
        fs::remove_file(workspace.join("bad-link")).unwrap();
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-other",
                "restore-b",
                false,
                &mut |_| Ok(()),
            )
            .is_err()
        );
        fs::hard_link(
            workspace.join("journal.md"),
            workspace.join("linked-journal"),
        )
        .unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                "restore-c",
                false,
                &mut |_| Ok(()),
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_and_restore_fail_closed_on_special_files_and_tamper() {
        use nix::sys::stat::Mode;
        use nix::unistd::mkfifo;

        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        mkfifo(
            &workspace.join("device-like-fifo"),
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .unwrap();
        assert!(create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).is_err());
        fs::remove_file(workspace.join("device-like-fifo")).unwrap();
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();

        let sealed = snapshot.join("fixture/state/var/state.db");
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&sealed, b"tampered snapshot database").unwrap();
        assert!(verify_inner(&snapshot, "gen-old", false).is_err());
    }

    #[test]
    fn restore_rejects_a_tampered_crash_journal() {
        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
        fs::write(workspace.join("journal.md"), b"candidate").unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                "restore-tamper",
                false,
                &mut |phase| {
                    if phase == "prepared" {
                        return Err(Error::new("simulated power loss"));
                    }
                    Ok(())
                },
            )
            .is_err()
        );
        fs::write(snapshots.join("restore-restore-tamper.json"), b"{}\n").unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                "restore-tamper",
                false,
                &mut |_| Ok(()),
            )
            .is_err()
        );
    }

    #[test]
    fn restore_recovers_interrupted_staging_and_root_swap() {
        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
        fs::write(workspace.join("journal.md"), b"candidate").unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                "restore-interrupted",
                false,
                &mut |phase| {
                    if phase == "prepared" {
                        return Err(Error::new("simulated power loss"));
                    }
                    Ok(())
                },
            )
            .is_err()
        );
        let journal: RestoreJournal = serde_json::from_slice(
            &fs::read(snapshots.join("restore-restore-interrupted.json")).unwrap(),
        )
        .unwrap();
        let state = temp.path().join("state");
        let live = state.join("home/default/edge");
        let old = staging_path(&state, "state/home/default/edge", &journal, "old").unwrap();
        fs::rename(&live, &old).unwrap();

        restore_inner(
            &workspace,
            &snapshot,
            "gen-old",
            "restore-interrupted",
            false,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(workspace.join("journal.md")).unwrap(),
            b"baseline journal"
        );
    }

    #[test]
    fn restore_rebuilds_only_its_exact_partial_staging_root() {
        let temp = tempfile::tempdir().unwrap();
        let (workspace, snapshots) = fixture(temp.path());
        let snapshot = snapshots.join("baseline");
        create_inner(&workspace, &snapshot, "gen-old", &"a".repeat(64), false).unwrap();
        fs::write(workspace.join("journal.md"), b"candidate").unwrap();
        assert!(
            restore_inner(
                &workspace,
                &snapshot,
                "gen-old",
                "restore-partial",
                false,
                &mut |phase| {
                    if phase == "journal_bound" {
                        return Err(Error::new("simulated disk interruption"));
                    }
                    Ok(())
                },
            )
            .is_err()
        );
        let journal: RestoreJournal = serde_json::from_slice(
            &fs::read(snapshots.join("restore-restore-partial.json")).unwrap(),
        )
        .unwrap();
        let state = temp.path().join("state");
        let partial = staging_path(&state, "state/home/default/edge", &journal, "new").unwrap();
        fs::create_dir(&partial).unwrap();
        fs::write(partial.join("partial-copy"), b"interrupted").unwrap();

        restore_inner(
            &workspace,
            &snapshot,
            "gen-old",
            "restore-partial",
            false,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(workspace.join("journal.md")).unwrap(),
            b"baseline journal"
        );
    }
}
