//! Generic JSON / JSONL persistence helpers and the continuity-index I/O for
//! [`ActionContinuityStore`].
//!
//! Extracted verbatim from the monolithic `action_continuity.rs` as part of the
//! decomposition roadmap (item 2, tranche A1). Behavior-identical: pure
//! filesystem helpers over `self.root`. See
//! `docs/steward-notes/ARCHITECTURE_DECOMPOSITION_PLAN_2026-06-13.md`.

use super::*;

impl ActionContinuityStore {
    pub(super) fn load_index(&self) -> Result<ContinuityIndex> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(ContinuityIndex::default());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub(super) fn save_index(&self, index: &ContinuityIndex) -> Result<()> {
        self.write_json(&self.index_path(), index)
    }

    pub(super) fn append_jsonl<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(value)?)?;
        Ok(())
    }

    pub(super) fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)
            .with_context(|| format!("writing {}", path.display()))
    }
}
