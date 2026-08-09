//! Immutable CPU-edge checkpoint implementation.

#![allow(clippy::missing_errors_doc)]

pub mod checkpoint;
pub mod flush;
pub mod snapshot;

pub use astrid_edge_rescue_helper::{Error, Result};

use std::path::{Path, PathBuf};

pub(crate) const AUTHORITY: &str =
    "immutable_rescue_evidence_not_astrid_authorship_or_mutable_runtime_claim";

pub(crate) fn state_root(workspace: &Path) -> Result<PathBuf> {
    let workspace = workspace
        .canonicalize()
        .map_err(|error| Error::new(format!("cannot resolve workspace: {error}")))?;
    let suffix = Path::new("home/default/edge");
    if !workspace.ends_with(suffix) {
        return Err(Error::new(
            "workspace is not the exact state/home/default/edge layout",
        ));
    }
    let root = workspace
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error::new("workspace state root cannot be derived"))?;
    if root.as_os_str().is_empty() || root == Path::new("/") {
        return Err(Error::new("derived state root is unsafe"));
    }
    Ok(root.to_path_buf())
}

pub(crate) fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

pub(crate) fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
