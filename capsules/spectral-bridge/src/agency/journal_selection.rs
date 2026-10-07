//! Own-journal candidates for agency selection; peer mirrors remain retrievable elsewhere.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(super) fn local_journal_entries(journal_dir: &Path) -> Vec<(PathBuf, SystemTime, String)> {
    let mut entries: Vec<(PathBuf, SystemTime, String)> = fs::read_dir(journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "txt") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            let content = fs::read_to_string(&path).ok()?;
            if crate::journal::is_peer_mirror_record(&content) {
                return None;
            }
            Some((path, modified, content))
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
}
