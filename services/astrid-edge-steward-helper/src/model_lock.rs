use std::fs::{File, OpenOptions};
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::{Error, Result};

/// Open the shared provider lock only after verifying its stable identity and
/// immutable installer ownership contract.
pub(crate) fn open(config: &Config) -> Result<File> {
    let before = std::fs::symlink_metadata(&config.model_lock)?;
    #[cfg(debug_assertions)]
    let required_uid = if config.appliance_id == "test-appliance" {
        before.uid()
    } else {
        0
    };
    #[cfg(not(debug_assertions))]
    let required_uid = 0;
    validate_parent(
        config
            .model_lock
            .parent()
            .ok_or_else(|| Error::new("shared model lock has no parent"))?,
        required_uid,
    )?;
    validate_metadata(&before, required_uid)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000 | 0o02_000_000);
    let file = options.open(&config.model_lock)?;
    let opened = file.metadata()?;
    let after = std::fs::symlink_metadata(&config.model_lock)?;
    validate_metadata(&opened, required_uid)?;
    validate_metadata(&after, required_uid)?;
    validate_identity(&before, &opened, &after)?;
    Ok(file)
}

fn validate_identity(
    before: &std::fs::Metadata,
    opened: &std::fs::Metadata,
    after: &std::fs::Metadata,
) -> Result<()> {
    let identity = |metadata: &std::fs::Metadata| (metadata.dev(), metadata.ino());
    if identity(before) != identity(opened) || identity(opened) != identity(after) {
        return Err(Error::new(
            "shared model lock was replaced across its verified open",
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: &std::fs::Metadata, required_uid: u32) -> Result<()> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != required_uid
        || metadata.permissions().mode() & 0o777 != 0o640
    {
        return Err(Error::new(
            "shared model lock must be owner-controlled, regular, single-linked, and mode 0640",
        ));
    }
    Ok(())
}

fn validate_parent(parent: &Path, required_uid: u32) -> Result<()> {
    if !parent.is_absolute() {
        return Err(Error::new("shared model lock parent must be absolute"));
    }
    let mut cursor = PathBuf::new();
    for component in parent.components() {
        cursor.push(component);
        if cursor == Path::new("/") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&cursor)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || ![0, required_uid].contains(&metadata.uid())
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(Error::new(
                "shared model lock parent traverses a linked or writable directory",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, hard_link};
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

    use super::{validate_identity, validate_metadata, validate_parent};

    #[test]
    fn rejects_world_writable_parent_ancestor() {
        let fixture_parent = fs::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
        let temporary = tempfile::tempdir_in(fixture_parent).unwrap();
        let unsafe_parent = temporary.path().join("unsafe-model-lock-parent");
        fs::create_dir(&unsafe_parent).unwrap();
        let required_uid = fs::metadata(&unsafe_parent).unwrap().uid();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o707)).unwrap();

        let error = validate_parent(&unsafe_parent, required_uid).unwrap_err();
        assert_eq!(
            error.to_string(),
            "shared model lock parent traverses a linked or writable directory"
        );
    }

    #[test]
    fn rejects_links_world_access_and_replacement() {
        let temporary = tempfile::tempdir().unwrap();
        let first = temporary.path().join("first.lock");
        let second = temporary.path().join("second.lock");
        fs::write(&first, b"").unwrap();
        fs::write(&second, b"").unwrap();
        fs::set_permissions(&first, fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(&second, fs::Permissions::from_mode(0o640)).unwrap();
        let first_metadata = fs::metadata(&first).unwrap();
        assert!(validate_metadata(&first_metadata, first_metadata.uid()).is_ok());
        fs::set_permissions(&first, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(validate_metadata(&fs::metadata(&first).unwrap(), first_metadata.uid()).is_err());
        fs::set_permissions(&first, fs::Permissions::from_mode(0o640)).unwrap();
        hard_link(&first, temporary.path().join("hard.lock")).unwrap();
        assert!(validate_metadata(&fs::metadata(&first).unwrap(), first_metadata.uid()).is_err());
        let linked = temporary.path().join("linked.lock");
        symlink(&second, &linked).unwrap();
        assert!(
            validate_metadata(
                &fs::symlink_metadata(&linked).unwrap(),
                first_metadata.uid()
            )
            .is_err()
        );
        assert!(
            validate_identity(
                &first_metadata,
                &first_metadata,
                &fs::metadata(&second).unwrap()
            )
            .is_err()
        );
    }
}
