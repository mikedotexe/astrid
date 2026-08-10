//! Bounded filesystem primitives. Every caller deals in IDs; paths remain private here.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{
        DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::{
    digest::{sha256_hex, validate_sha256, validate_signature_hex},
    types::{
        BrokerError, BrokerResult, MAX_SOURCE_FILE_BYTES, MAX_SOURCE_FILES, MAX_SOURCE_TOTAL_BYTES,
        SOURCE_BINDING_SCHEMA_V1, SignedSourceBindingV1, SignedSourceRootV1, SourceEntry,
    },
};

const SOURCE_MANIFEST_SCHEMA: &str = "astrid.edge.local_source_manifest.v1";
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(crate) struct IndexedSourceFile {
    pub source_file_id: String,
    pub relative_path: PathBuf,
    pub basename: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub line_count: usize,
}

impl IndexedSourceFile {
    pub(crate) fn public_entry(&self) -> SourceEntry {
        SourceEntry {
            source_file_id: self.source_file_id.clone(),
            basename: self.basename.clone(),
            sha256: self.sha256.clone(),
            size_bytes: self.size_bytes,
            line_count: self.line_count,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceIndex {
    pub root: PathBuf,
    pub source_id: String,
    pub manifest_sha256: String,
    pub files: Vec<IndexedSourceFile>,
}

impl SourceIndex {
    pub(crate) fn load(source: &SignedSourceRootV1) -> BrokerResult<Self> {
        validate_source_binding(source)?;
        let index = build_source_index(&source.root, &source.expected_source_id)?;
        if index.manifest_sha256 != source.expected_manifest_sha256 {
            return Err(BrokerError::Stale("source manifest digest changed"));
        }
        Ok(index)
    }

    pub(crate) fn verify_current(&self) -> BrokerResult<Self> {
        let current = build_source_index(&self.root, &self.source_id)?;
        if current.manifest_sha256 != self.manifest_sha256 {
            return Err(BrokerError::Stale("source root changed after attestation"));
        }
        Ok(current)
    }

    pub(crate) fn file(&self, source_file_id: &str) -> BrokerResult<&IndexedSourceFile> {
        validate_id(source_file_id, "source file id")?;
        self.files
            .iter()
            .find(|file| file.source_file_id == source_file_id)
            .ok_or(BrokerError::NotFound("unknown source file id"))
    }

    pub(crate) fn read_file(&self, file: &IndexedSourceFile) -> BrokerResult<String> {
        let path = self.root.join(&file.relative_path);
        let text = read_bounded_text(&path, MAX_SOURCE_FILE_BYTES, false)?;
        if sha256_hex(text.as_bytes()) != file.sha256 {
            return Err(BrokerError::Stale("source file hash changed"));
        }
        Ok(text)
    }
}

pub(crate) fn signed_binding_payload_sha256(
    source_id: &str,
    manifest_sha256: &str,
) -> BrokerResult<String> {
    validate_id(source_id, "source id")?;
    require_sha256(manifest_sha256, "source manifest digest")?;
    let payload = format!(
        "schema={SOURCE_BINDING_SCHEMA_V1}\nsource_id={source_id}\nmanifest_sha256={manifest_sha256}\n"
    );
    Ok(sha256_hex(payload.as_bytes()))
}

pub(crate) fn compute_source_manifest_sha256(root: &Path, source_id: &str) -> BrokerResult<String> {
    Ok(build_source_index(root, source_id)?.manifest_sha256)
}

fn validate_source_binding(source: &SignedSourceRootV1) -> BrokerResult<()> {
    validate_id(&source.expected_source_id, "source id")?;
    require_sha256(&source.expected_manifest_sha256, "source manifest digest")?;
    let SignedSourceBindingV1 {
        schema,
        signer_key_id,
        signature_hex,
        signed_payload_sha256,
    } = &source.binding;
    if schema != SOURCE_BINDING_SCHEMA_V1 {
        return Err(BrokerError::InvalidInput(
            "unsupported source binding schema",
        ));
    }
    validate_label(signer_key_id, "signer key id")?;
    if !validate_signature_hex(signature_hex) {
        return Err(BrokerError::InvalidInput(
            "invalid source signature encoding",
        ));
    }
    let expected = signed_binding_payload_sha256(
        &source.expected_source_id,
        &source.expected_manifest_sha256,
    )?;
    if signed_payload_sha256 != &expected {
        return Err(BrokerError::Integrity(
            "source signature binding does not match source identity and manifest",
        ));
    }
    Ok(())
}

fn build_source_index(root: &Path, source_id: &str) -> BrokerResult<SourceIndex> {
    validate_id(source_id, "source id")?;
    let root_metadata = fs::symlink_metadata(root)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(BrokerError::SecurityViolation(
            "source root must be a real directory",
        ));
    }
    let mut pending = vec![(PathBuf::new(), root.to_path_buf())];
    let mut files = Vec::new();
    let mut total_bytes = 0_u64;
    while let Some((relative_directory, directory)) = pending.pop() {
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| BrokerError::SecurityViolation("non-UTF-8 source name"))?;
            validate_component(&name)?;
            let relative = relative_directory.join(&name);
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(BrokerError::SecurityViolation(
                    "source tree contains a symlink",
                ));
            }
            if metadata.is_dir() {
                pending.push((relative, path));
                continue;
            }
            if !metadata.is_file() {
                return Err(BrokerError::SecurityViolation(
                    "source tree contains a non-regular entry",
                ));
            }
            if metadata.nlink() != 1 {
                return Err(BrokerError::SecurityViolation(
                    "source tree contains a hard-linked file",
                ));
            }
            if files.len() >= MAX_SOURCE_FILES {
                return Err(BrokerError::LimitExceeded("source file count"));
            }
            if metadata.len() > MAX_SOURCE_FILE_BYTES {
                return Err(BrokerError::LimitExceeded("source file bytes"));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or(BrokerError::LimitExceeded("source total bytes"))?;
            if total_bytes > MAX_SOURCE_TOTAL_BYTES {
                return Err(BrokerError::LimitExceeded("source total bytes"));
            }
            let text = read_bounded_text(&path, MAX_SOURCE_FILE_BYTES, false)?;
            let digest = sha256_hex(text.as_bytes());
            let identifier_seed = format!(
                "source_id={source_id}\nrelative={}\nsha256={digest}\n",
                relative.to_string_lossy()
            );
            let identifier_digest = sha256_hex(identifier_seed.as_bytes());
            let source_file_id = format!("src-{}", &identifier_digest[..24]);
            files.push(IndexedSourceFile {
                source_file_id,
                relative_path: relative,
                basename: name,
                sha256: digest,
                size_bytes: metadata.len(),
                line_count: text.lines().count(),
            });
        }
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let unique = files
        .iter()
        .map(|file| file.source_file_id.as_str())
        .collect::<BTreeSet<_>>();
    if unique.len() != files.len() {
        return Err(BrokerError::Integrity("source file ID collision"));
    }
    let mut manifest = format!("schema={SOURCE_MANIFEST_SCHEMA}\nsource_id={source_id}\n");
    for file in &files {
        writeln!(
            manifest,
            "file={}|{}|{}|{}|{}",
            file.source_file_id,
            file.relative_path.to_string_lossy(),
            file.sha256,
            file.size_bytes,
            file.line_count
        )
        .expect("writing to a String cannot fail");
    }
    Ok(SourceIndex {
        root: root.to_path_buf(),
        source_id: source_id.to_string(),
        manifest_sha256: sha256_hex(manifest.as_bytes()),
        files,
    })
}

pub(crate) fn read_bounded_text(
    path: &Path,
    maximum_bytes: u64,
    require_private: bool,
) -> BrokerResult<String> {
    let before = fs::symlink_metadata(path)?;
    validate_regular_metadata(&before, maximum_bytes, require_private)?;
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    validate_regular_metadata(&opened, maximum_bytes, require_private)?;
    if before.dev() != opened.dev() || before.ino() != opened.ino() {
        return Err(BrokerError::Stale("file identity changed while opening"));
    }
    let capacity =
        usize::try_from(opened.len()).map_err(|_| BrokerError::LimitExceeded("file bytes"))?;
    let mut bytes = Vec::with_capacity(capacity);
    (&mut file)
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(BrokerError::LimitExceeded("file bytes"));
    }
    let after = file.metadata()?;
    if opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || opened.mtime() != after.mtime()
        || opened.mtime_nsec() != after.mtime_nsec()
    {
        return Err(BrokerError::Stale("file changed while reading"));
    }
    validate_text_bytes(&bytes)?;
    String::from_utf8(bytes).map_err(|_| BrokerError::SecurityViolation("binary source file"))
}

fn validate_regular_metadata(
    metadata: &fs::Metadata,
    maximum_bytes: u64,
    require_private: bool,
) -> BrokerResult<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(BrokerError::SecurityViolation("expected a regular file"));
    }
    if metadata.nlink() != 1 {
        return Err(BrokerError::SecurityViolation("hard-linked file rejected"));
    }
    if metadata.len() > maximum_bytes {
        return Err(BrokerError::LimitExceeded("file bytes"));
    }
    if require_private && metadata.mode() & 0o077 != 0 {
        return Err(BrokerError::SecurityViolation(
            "private file permissions widened",
        ));
    }
    Ok(())
}

pub(crate) fn validate_text_bytes(bytes: &[u8]) -> BrokerResult<()> {
    if bytes.contains(&0)
        || bytes
            .iter()
            .any(|byte| (*byte < b' ' && !matches!(*byte, b'\n' | b'\r' | b'\t')) || *byte == 0x7f)
        || std::str::from_utf8(bytes).is_err()
    {
        return Err(BrokerError::SecurityViolation(
            "binary text payload rejected",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_private_root(root: &Path) -> BrokerResult<()> {
    if !root.exists() {
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder.create(root)?;
    }
    ensure_private_directory(root)
}

pub(crate) fn ensure_private_child(parent: &Path, name: &str) -> BrokerResult<PathBuf> {
    validate_component(name)?;
    ensure_private_directory(parent)?;
    let child = parent.join(name);
    if !child.exists() {
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder.create(&child)?;
    }
    ensure_private_directory(&child)?;
    Ok(child)
}

pub(crate) fn ensure_private_directory(path: &Path) -> BrokerResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(BrokerError::SecurityViolation(
            "candidate directory must be private and link-free",
        ));
    }
    Ok(())
}

pub(crate) fn atomic_private_write(path: &Path, bytes: &[u8]) -> BrokerResult<()> {
    validate_text_bytes(bytes)?;
    let parent = path
        .parent()
        .ok_or(BrokerError::InvalidInput("private path has no parent"))?;
    ensure_private_directory(parent)?;
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        validate_regular_metadata(&metadata, u64::MAX, true)?;
    }
    let mut temporary = None;
    for _ in 0..16 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!("tmp-{}-{sequence}", std::process::id()));
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&candidate);
        match opened {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
            Err(error) => return Err(error.into()),
        }
    }
    let (temporary_path, mut file) = temporary.ok_or(BrokerError::Conflict(
        "cannot allocate atomic temporary file",
    ))?;
    let result = (|| -> BrokerResult<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary_path, path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        drop(fs::remove_file(&temporary_path));
    }
    result
}

pub(crate) fn validate_id(value: &str, label: &'static str) -> BrokerResult<()> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || value.starts_with('.')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(BrokerError::InvalidValue(format!("invalid {label}")));
    }
    Ok(())
}

pub(crate) fn validate_label(value: &str, label: &'static str) -> BrokerResult<()> {
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('.')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(BrokerError::InvalidValue(format!("invalid {label}")));
    }
    Ok(())
}

pub(crate) fn validate_component(value: &str) -> BrokerResult<()> {
    if value.is_empty()
        || value.len() > 255
        || value == "."
        || value == ".."
        || value.starts_with('.')
        || value.contains('/')
        || value.contains('\\')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(BrokerError::SecurityViolation(
            "hidden or unsafe source component",
        ));
    }
    Ok(())
}

pub(crate) fn require_sha256(value: &str, label: &'static str) -> BrokerResult<()> {
    if !validate_sha256(value) {
        return Err(BrokerError::InvalidValue(format!("invalid {label}")));
    }
    Ok(())
}
