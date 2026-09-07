use std::path::Path;

use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone)]
pub(super) struct CatalogedSourceV3 {
    pub(super) source_identity: String,
    pub(super) source_sha256: String,
    pub(super) source_bytes: usize,
    pub(super) source_lines: usize,
}

pub(super) fn catalog_source(path: &Path, content: &str, source_lines: usize) -> CatalogedSourceV3 {
    CatalogedSourceV3 {
        source_identity: source_identity(path),
        source_sha256: sha256_bytes(content.as_bytes()),
        source_bytes: content.len(),
        source_lines,
    }
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_identity(path: &Path) -> String {
    let paths = crate::paths::bridge_paths();
    for (owner, root) in [
        ("astrid", paths.astrid_root()),
        ("minime", paths.minime_root()),
    ] {
        if let Ok(relative) = path.strip_prefix(root) {
            return format!("{owner}/{}", relative.display());
        }
    }
    let display = path.display().to_string();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown_source");
    format!(
        "external/{}:{name}",
        &sha256_bytes(display.as_bytes())[..16]
    )
}
