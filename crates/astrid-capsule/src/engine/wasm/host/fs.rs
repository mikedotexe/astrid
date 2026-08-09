use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use cap_fs_ext::{DirExt as _, FollowSymlinks, MetadataExt as _, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata, OpenOptions};

use crate::engine::wasm::bindings::astrid::capsule::fs;
use crate::engine::wasm::bindings::astrid::capsule::types::{
    BoundedFileRead, BoundedFileReadMode, FileEntryKind, FileStat, NoFollowFileStat,
};
use crate::engine::wasm::host::util;
use crate::engine::wasm::host_state::HostState;

/// URI scheme prefix for the principal's home directory.
const HOME_SCHEME: &str = "home://";

/// URI scheme prefix for the daemon's current working directory.
const CWD_SCHEME: &str = "cwd://";

/// Path prefix that maps to the principal's tmp directory.
const TMP_PREFIX: &str = "/tmp/";

/// Immutable host ceiling for either a whole-file or tail no-follow read.
const MAX_NOFOLLOW_READ_BYTES: u64 = 64 * 1024;

/// Strip any leading absolute slashes or prefixes (e.g. C:\) from the requested path
fn make_relative(requested: &str) -> &Path {
    let path = Path::new(requested);
    let mut components = path.components();
    while let Some(c) = components.clone().next() {
        if matches!(c, Component::RootDir | Component::Prefix(_)) {
            components.next(); // consume it
        } else {
            break;
        }
    }
    components.as_path()
}

/// Result of resolving a path to a physical absolute location on disk.
struct ResolvedPhysical {
    /// The fully resolved physical path (symlinks canonicalized where possible).
    physical: PathBuf,
    /// The canonical root this path was resolved against.
    canonical_root: PathBuf,
}

/// Compute the true physical absolute path for the security gate by canonicalizing on the host filesystem.
/// This prevents symlink bypass attacks where a lexical path passes the gate but cap-std follows a symlink.
fn resolve_physical_absolute(root: &Path, requested: &str) -> Result<ResolvedPhysical, String> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let relative_requested = make_relative(requested);
    let joined = canonical_root.join(relative_requested);

    let mut current_check = joined.clone();
    let mut unexisting_components = Vec::new();

    loop {
        if std::fs::symlink_metadata(&current_check).is_ok() {
            let canonical =
                std::fs::canonicalize(&current_check).unwrap_or_else(|_| current_check.clone());
            let mut final_path = canonical;
            for comp in unexisting_components.into_iter().rev() {
                final_path.push(comp);
            }
            if !final_path.starts_with(&canonical_root) {
                return Err(format!(
                    "path escapes root boundary: {requested} resolves to {}",
                    final_path.display()
                ));
            }
            return Ok(ResolvedPhysical {
                physical: final_path,
                canonical_root,
            });
        }
        if let Some(parent) = current_check.parent() {
            if let Some(file_name) = current_check.file_name() {
                unexisting_components.push(file_name.to_os_string());
            }
            current_check = parent.to_path_buf();
        } else {
            break;
        }
    }

    if !joined.starts_with(&canonical_root) {
        return Err(format!(
            "path escapes root boundary: {requested} resolves to {}",
            joined.display()
        ));
    }

    Ok(ResolvedPhysical {
        physical: joined,
        canonical_root,
    })
}

/// Which VFS target a resolved path points at.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VfsTarget {
    /// The workspace overlay VFS (default).
    Workspace,
    /// The principal's home directory (`home://`).
    Home,
    /// The principal's tmp directory (`/tmp/`).
    Tmp,
}

/// First-phase resolution result: physical path for the security gate,
/// the VFS-relative path, and which VFS to target.
struct ResolvedPath {
    /// Absolute physical path (for security gate check).
    physical: PathBuf,
    /// Path relative to the root (for VFS operations).
    relative: PathBuf,
    /// Which VFS this path targets.
    target: VfsTarget,
}

/// Second-phase resolution result: the VFS instance and capability handle
/// to use for the actual filesystem operation.
struct ResolvedVfsPath {
    /// Path relative to the VFS root.
    relative: PathBuf,
    /// The VFS instance to use.
    vfs: Arc<dyn astrid_vfs::Vfs>,
    /// The capability handle for the VFS root.
    handle: astrid_capabilities::DirHandle,
}

/// Phase 1: Resolve a raw guest path to a physical path and determine
/// whether it targets the workspace or home VFS.
fn resolve_path(state: &HostState, raw_path: &str) -> Result<ResolvedPath, String> {
    if let Some(stripped) = raw_path.strip_prefix(CWD_SCHEME) {
        let resolved = resolve_physical_absolute(&state.workspace_root, stripped)?;
        let relative = resolved
            .physical
            .strip_prefix(&resolved.canonical_root)
            .map_err(|_| "resolved cwd path escaped canonical root".to_string())?
            .to_path_buf();
        Ok(ResolvedPath {
            physical: resolved.physical,
            relative,
            target: VfsTarget::Workspace,
        })
    } else if let Some(stripped) = raw_path.strip_prefix(HOME_SCHEME) {
        let home_root = state.home_root.as_ref().ok_or_else(|| {
            "home:// scheme is not available: no home directory is configured. \
             Create the directory and restart the kernel."
                .to_string()
        })?;
        let resolved = resolve_physical_absolute(home_root, stripped)?;
        let relative = resolved
            .physical
            .strip_prefix(&resolved.canonical_root)
            .map_err(|_| "resolved home path escaped canonical root".to_string())?
            .to_path_buf();
        Ok(ResolvedPath {
            physical: resolved.physical,
            relative,
            target: VfsTarget::Home,
        })
    } else if raw_path.starts_with(TMP_PREFIX) || raw_path == "/tmp" {
        let tmp_root = state.tmp_dir.as_ref().ok_or_else(|| {
            "/tmp is not available: no tmp directory is configured for this principal.".to_string()
        })?;
        let stripped = raw_path
            .strip_prefix(TMP_PREFIX)
            .or_else(|| raw_path.strip_prefix("/tmp"))
            .unwrap_or("");
        let resolved = resolve_physical_absolute(tmp_root, stripped)?;
        let relative = resolved
            .physical
            .strip_prefix(&resolved.canonical_root)
            .map_err(|_| "resolved /tmp path escaped canonical root".to_string())?
            .to_path_buf();
        Ok(ResolvedPath {
            physical: resolved.physical,
            relative,
            target: VfsTarget::Tmp,
        })
    } else {
        let resolved = resolve_physical_absolute(&state.workspace_root, raw_path)?;
        let relative = resolved
            .physical
            .strip_prefix(&resolved.canonical_root)
            .map_err(|_| "resolved path escaped canonical root".to_string())?
            .to_path_buf();
        Ok(ResolvedPath {
            physical: resolved.physical,
            relative,
            target: VfsTarget::Workspace,
        })
    }
}

/// Phase 2: Given a first-phase result, select the correct VFS instance
/// and capability handle.
fn resolve_vfs(state: &HostState, resolved: &ResolvedPath) -> Result<ResolvedVfsPath, String> {
    match resolved.target {
        VfsTarget::Home => {
            let vfs = state.home_vfs.clone().ok_or_else(|| {
                "home:// VFS is not mounted. \
                 Create the directory and restart the kernel."
                    .to_string()
            })?;
            let handle = state
                .home_vfs_root_handle
                .clone()
                .ok_or_else(|| "home:// VFS root handle is not available".to_string())?;
            Ok(ResolvedVfsPath {
                relative: resolved.relative.clone(),
                vfs,
                handle,
            })
        },
        VfsTarget::Tmp => {
            let vfs = state
                .tmp_vfs
                .clone()
                .ok_or_else(|| "/tmp VFS is not mounted for this principal.".to_string())?;
            let handle = state
                .tmp_vfs_root_handle
                .clone()
                .ok_or_else(|| "/tmp VFS root handle is not available".to_string())?;
            Ok(ResolvedVfsPath {
                relative: resolved.relative.clone(),
                vfs,
                handle,
            })
        },
        VfsTarget::Workspace => Ok(ResolvedVfsPath {
            relative: resolved.relative.clone(),
            vfs: state.vfs.clone(),
            handle: state.vfs_root_handle.clone(),
        }),
    }
}

/// A final path entry reached through directory handles without following any
/// symbolic-link component.
struct NoFollowEntry {
    parent: Dir,
    basename: OsString,
    physical: PathBuf,
    metadata: Metadata,
}

fn no_follow_root_and_relative(
    state: &HostState,
    raw_path: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let (root, requested, direct_host_backing) =
        if let Some(stripped) = raw_path.strip_prefix(CWD_SCHEME) {
            (&state.workspace_root, stripped, state.overlay_vfs.is_none())
        } else if let Some(stripped) = raw_path.strip_prefix(HOME_SCHEME) {
            (
                state.home_root.as_ref().ok_or_else(|| {
                    "home:// scheme is not available: no home directory is configured".to_string()
                })?,
                stripped,
                state.home_vfs.is_some(),
            )
        } else if raw_path.starts_with(TMP_PREFIX) || raw_path == "/tmp" {
            (
                state
                    .tmp_dir
                    .as_ref()
                    .ok_or_else(|| "/tmp is not available for this principal".to_string())?,
                raw_path
                    .strip_prefix(TMP_PREFIX)
                    .or_else(|| raw_path.strip_prefix("/tmp"))
                    .unwrap_or(""),
                state.tmp_vfs.is_some(),
            )
        } else {
            (&state.workspace_root, raw_path, state.overlay_vfs.is_none())
        };
    if !direct_host_backing {
        return Err(
            "no-follow inspection is unavailable for a non-direct or overlay VFS".to_string(),
        );
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve no-follow capability root: {error}"))?;
    let requested = Path::new(requested);
    if requested.as_os_str().is_empty()
        || requested.is_absolute()
        || requested
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("no-follow paths must contain only relative normal components".to_string());
    }
    Ok((canonical_root, requested.to_path_buf()))
}

fn resolve_no_follow_entry(state: &HostState, raw_path: &str) -> Result<NoFollowEntry, String> {
    let (root, relative) = no_follow_root_and_relative(state, raw_path)?;
    resolve_no_follow_entry_at(&root, &relative)
}

fn resolve_no_follow_entry_at(root: &Path, relative: &Path) -> Result<NoFollowEntry, String> {
    let mut components = relative.components().peekable();
    let basename = components
        .next_back()
        .and_then(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .ok_or_else(|| "no-follow path has no final entry".to_string())?;
    let mut parent = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| format!("cannot open no-follow capability root: {error}"))?;
    for component in components {
        let Component::Normal(name) = component else {
            return Err("no-follow path contains a non-normal component".to_string());
        };
        parent = parent
            .open_dir_nofollow(name)
            .map_err(|_| "no-follow path has a symlink or non-directory component".to_string())?;
    }
    let metadata = parent
        .symlink_metadata(&basename)
        .map_err(|error| format!("cannot inspect no-follow entry: {error}"))?;
    Ok(NoFollowEntry {
        parent,
        physical: root.join(relative),
        basename,
        metadata,
    })
}

fn metadata_mtime(metadata: &Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .map(cap_std::time::SystemTime::into_std)
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn entry_kind(metadata: &Metadata) -> FileEntryKind {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        FileEntryKind::Symlink
    } else if metadata.is_file() {
        FileEntryKind::RegularFile
    } else if metadata.is_dir() {
        FileEntryKind::Directory
    } else {
        FileEntryKind::Other
    }
}

fn no_follow_stat(metadata: &Metadata) -> NoFollowFileStat {
    NoFollowFileStat {
        size: metadata.len(),
        kind: entry_kind(metadata),
        mtime: metadata_mtime(metadata),
        hard_link_count: metadata.nlink(),
    }
}

fn stable_identity(metadata: &Metadata) -> (u64, u64, u64, u64, Option<(u64, u32)>) {
    let modified = metadata
        .modified()
        .ok()
        .map(cap_std::time::SystemTime::into_std)
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()));
    (
        metadata.dev(),
        metadata.ino(),
        metadata.nlink(),
        metadata.len(),
        modified,
    )
}

fn read_region(
    file: &mut cap_std::fs::File,
    offset: u64,
    length: usize,
) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("cannot seek bounded no-follow file: {error}"))?;
    let mut data = vec![0_u8; length];
    file.read_exact(&mut data)
        .map_err(|error| format!("cannot read stable bounded no-follow region: {error}"))?;
    Ok(data)
}

fn read_bounded_no_follow_entry(
    entry: NoFollowEntry,
    maximum_bytes: u64,
    mode: BoundedFileReadMode,
) -> Result<BoundedFileRead, String> {
    if maximum_bytes == 0 || maximum_bytes > MAX_NOFOLLOW_READ_BYTES {
        return Err(format!(
            "bounded no-follow read must request 1-{MAX_NOFOLLOW_READ_BYTES} bytes"
        ));
    }
    if !entry.metadata.is_file()
        || entry.metadata.file_type().is_symlink()
        || entry.metadata.nlink() != 1
    {
        return Err(
            "bounded no-follow reads require a non-symlink regular file with one hard link"
                .to_string(),
        );
    }
    let captured_size = entry.metadata.len();
    let offset = match mode {
        BoundedFileReadMode::Whole => {
            if captured_size > maximum_bytes {
                return Err("whole-file no-follow read exceeds the requested bound".to_string());
            }
            0
        },
        BoundedFileReadMode::Tail => captured_size.saturating_sub(maximum_bytes),
    };
    let length = usize::try_from(captured_size.saturating_sub(offset))
        .map_err(|_| "bounded no-follow region cannot be represented".to_string())?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = entry
        .parent
        .open_with(&entry.basename, &options)
        .map_err(|error| format!("cannot open bounded no-follow file: {error}"))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened no-follow file: {error}"))?;
    if stable_identity(&entry.metadata) != stable_identity(&opened)
        || !opened.is_file()
        || opened.nlink() != 1
    {
        return Err("bounded no-follow file identity changed before reading".to_string());
    }
    let starts_at_line_boundary = if offset == 0 {
        true
    } else {
        read_region(&mut file, offset.saturating_sub(1), 1)? == b"\n"
    };
    let first = read_region(&mut file, offset, length)?;
    let second = read_region(&mut file, offset, length)?;
    if first != second {
        return Err("bounded no-follow file changed while reading".to_string());
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect opened no-follow file: {error}"))?;
    let path_after = entry
        .parent
        .symlink_metadata(&entry.basename)
        .map_err(|error| format!("cannot re-inspect no-follow path: {error}"))?;
    if stable_identity(&opened) != stable_identity(&after)
        || stable_identity(&after) != stable_identity(&path_after)
        || path_after.file_type().is_symlink()
        || !path_after.is_file()
        || path_after.nlink() != 1
    {
        return Err("bounded no-follow file identity changed during reading".to_string());
    }
    Ok(BoundedFileRead {
        data: first,
        offset,
        captured_size,
        starts_at_line_boundary,
    })
}

impl fs::Host for HostState {
    fn fs_exists(&mut self, path: String) -> Result<bool, String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        // Phase 1: resolve to physical path
        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied exists check: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        let exists = util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            vfs_path
                .vfs
                .exists(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await
        })
        .unwrap_or(false);

        Ok(exists)
    }

    fn fs_mkdir(&mut self, path: String) -> Result<(), String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_write(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied mkdir: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            vfs_path
                .vfs
                .mkdir(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await
        })
        .map_err(|e| format!("mkdir failed: {e}"))
    }

    fn fs_readdir(&mut self, path: String) -> Result<Vec<String>, String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied readdir: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        let entries = util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            vfs_path
                .vfs
                .readdir(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await
        })
        .map_err(|e| format!("readdir failed: {e}"))?;

        Ok(entries.into_iter().map(|e| e.name).collect())
    }

    fn fs_stat(&mut self, path: String) -> Result<FileStat, String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied stat: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        let metadata = util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            vfs_path
                .vfs
                .stat(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await
        })
        .map_err(|e| format!("stat failed: {e}"))?;

        Ok(FileStat {
            size: metadata.size,
            is_dir: metadata.is_dir,
            mtime: Some(metadata.mtime),
        })
    }

    fn fs_lstat_nofollow(&mut self, path: String) -> Result<NoFollowFileStat, String> {
        let entry = resolve_no_follow_entry(self, &path)?;
        if let Some(gate) = self.security.clone() {
            let physical = entry.physical.to_string_lossy().to_string();
            let capsule_id = self.capsule_id.as_str().to_owned();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&capsule_id, &physical).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied no-follow stat: {reason}"));
            }
        }
        Ok(no_follow_stat(&entry.metadata))
    }

    fn fs_read_bounded_nofollow(
        &mut self,
        path: String,
        maximum_bytes: u64,
        mode: BoundedFileReadMode,
    ) -> Result<BoundedFileRead, String> {
        let entry = resolve_no_follow_entry(self, &path)?;
        if let Some(gate) = self.security.clone() {
            let physical = entry.physical.to_string_lossy().to_string();
            let capsule_id = self.capsule_id.as_str().to_owned();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&capsule_id, &physical).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied bounded no-follow read: {reason}"));
            }
        }
        read_bounded_no_follow_entry(entry, maximum_bytes, mode)
    }

    fn fs_unlink(&mut self, path: String) -> Result<(), String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_write(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied unlink: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            vfs_path
                .vfs
                .unlink(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await
        })
        .map_err(|e| format!("unlink failed: {e}"))
    }

    fn read_file(&mut self, path: String) -> Result<Vec<u8>, String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_read(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied read_file: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            let metadata = vfs_path
                .vfs
                .stat(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                )
                .await?;
            if metadata.size > util::MAX_GUEST_PAYLOAD_LEN {
                return Err(astrid_vfs::VfsError::PermissionDenied(format!(
                    "File too large to read into memory ({} bytes > {} bytes)",
                    metadata.size,
                    util::MAX_GUEST_PAYLOAD_LEN
                )));
            }

            let handle = vfs_path
                .vfs
                .open(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                    false,
                    false,
                )
                .await?;
            let data = vfs_path.vfs.read(&handle).await;
            let _ = vfs_path.vfs.close(&handle).await;
            data
        })
        .map_err(|e| format!("IO error: {e}"))
    }

    fn write_file(&mut self, path: String, content: Vec<u8>) -> Result<(), String> {
        let capsule_id = self.capsule_id.as_str().to_owned();

        let resolved = resolve_path(self, &path)?;

        let security = self.security.clone();
        if let Some(gate) = security {
            let p = resolved.physical.to_string_lossy().to_string();
            let pid = capsule_id.clone();
            let check =
                util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async move {
                    gate.check_file_write(&pid, &p).await
                });
            if let Err(reason) = check {
                return Err(format!("security denied write_file: {reason}"));
            }
        }

        let vfs_path = resolve_vfs(self, &resolved)?;

        util::bounded_block_on(&self.runtime_handle, &self.host_semaphore, async {
            // Note: pass truncate=true to emulate standard write behavior
            let handle = vfs_path
                .vfs
                .open(
                    &vfs_path.handle,
                    vfs_path.relative.to_string_lossy().as_ref(),
                    true,
                    true,
                )
                .await?;
            let res = vfs_path.vfs.write(&handle, &content).await;
            let _ = vfs_path.vfs.close(&handle).await;
            res
        })
        .map_err(|e| format!("write_file failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use cap_fs_ext::MetadataExt as _;

    use super::{
        BoundedFileReadMode, FileEntryKind, MAX_NOFOLLOW_READ_BYTES, read_bounded_no_follow_entry,
        resolve_no_follow_entry_at,
    };

    #[test]
    fn bounded_no_follow_whole_and_tail_reads_are_stable_and_capped() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("owned")).unwrap();
        std::fs::write(
            root.path().join("owned/ledger.jsonl"),
            b"first\nsecond\nthird\n",
        )
        .unwrap();

        let whole = read_bounded_no_follow_entry(
            resolve_no_follow_entry_at(root.path(), Path::new("owned/ledger.jsonl")).unwrap(),
            64,
            BoundedFileReadMode::Whole,
        )
        .unwrap();
        assert_eq!(whole.data, b"first\nsecond\nthird\n");
        assert_eq!(whole.offset, 0);
        assert!(whole.starts_at_line_boundary);

        let tail = read_bounded_no_follow_entry(
            resolve_no_follow_entry_at(root.path(), Path::new("owned/ledger.jsonl")).unwrap(),
            12,
            BoundedFileReadMode::Tail,
        )
        .unwrap();
        assert_eq!(tail.data, b"econd\nthird\n");
        assert_eq!(tail.offset, 7);
        assert!(!tail.starts_at_line_boundary);

        let oversized = read_bounded_no_follow_entry(
            resolve_no_follow_entry_at(root.path(), Path::new("owned/ledger.jsonl")).unwrap(),
            MAX_NOFOLLOW_READ_BYTES.saturating_add(1),
            BoundedFileReadMode::Tail,
        );
        assert!(oversized.is_err());
    }

    #[test]
    fn symlink_components_and_hardlinks_never_reach_file_bytes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("owned")).unwrap();
        std::fs::write(root.path().join("owned/secret.txt"), b"must-not-read").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("secret.txt", root.path().join("owned/link.txt")).unwrap();
            let link =
                resolve_no_follow_entry_at(root.path(), Path::new("owned/link.txt")).unwrap();
            assert_eq!(super::entry_kind(&link.metadata), FileEntryKind::Symlink);
            assert!(read_bounded_no_follow_entry(link, 64, BoundedFileReadMode::Whole).is_err());

            std::os::unix::fs::symlink("owned", root.path().join("owned-link")).unwrap();
            assert!(
                resolve_no_follow_entry_at(root.path(), Path::new("owned-link/secret.txt"))
                    .is_err()
            );
        }

        std::fs::hard_link(
            root.path().join("owned/secret.txt"),
            root.path().join("owned/hardlink.txt"),
        )
        .unwrap();
        let hardlink =
            resolve_no_follow_entry_at(root.path(), Path::new("owned/hardlink.txt")).unwrap();
        assert!(hardlink.metadata.nlink() > 1);
        assert!(read_bounded_no_follow_entry(hardlink, 64, BoundedFileReadMode::Whole).is_err());
    }

    #[test]
    fn path_replacement_between_inspection_and_open_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("value.txt"), b"original").unwrap();
        let inspected = resolve_no_follow_entry_at(root.path(), Path::new("value.txt")).unwrap();
        std::fs::rename(
            root.path().join("value.txt"),
            root.path().join("replaced.txt"),
        )
        .unwrap();
        std::fs::write(root.path().join("value.txt"), b"attacker").unwrap();
        assert!(read_bounded_no_follow_entry(inspected, 64, BoundedFileReadMode::Whole).is_err());
    }

    #[test]
    fn current_fs_linker_accepts_a_legacy_fs_interface_import() {
        const LEGACY_COMPONENT: &str = r#"
            (component
              (type $legacy-fs
                (instance
                  (type $exists-result (result bool (error string)))
                  (type $exists-func
                    (func (param "path" string) (result $exists-result)))
                  (export "fs-exists" (func (type $exists-func)))))
              (import "astrid:capsule/fs@0.1.0"
                (instance $legacy-fs-import (type $legacy-fs))))
        "#;

        let mut configuration = wasmtime::Config::new();
        configuration.wasm_component_model(true);
        let engine = wasmtime::Engine::new(&configuration).unwrap();
        let component = wasmtime::component::Component::new(&engine, LEGACY_COMPONENT).unwrap();
        let mut linker = wasmtime::component::Linker::<super::HostState>::new(&engine);
        crate::engine::wasm::bindings::Capsule::add_to_linker::<
            super::HostState,
            wasmtime::component::HasSelf<super::HostState>,
        >(&mut linker, |state| state)
        .unwrap();
        linker.instantiate_pre(&component).unwrap();
    }
}
